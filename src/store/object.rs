//! Sample storage abstraction
//!
//! Supports storing samples in Postgres (simple) or Object Store (scalable).
//! Object store supports S3, Azure Blob, GCS, MinIO, and Cloudflare R2.

use anyhow::{anyhow, Result};
use object_store::{aws::AmazonS3Builder, ObjectStore};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::{ObjectStoreConfig, StorageConfig};

/// Sample storage abstraction
#[derive(Clone)]
pub struct SampleStore {
    mode: StorageMode,
    db: PgPool,
}

#[derive(Clone)]
enum StorageMode {
    Postgres,
    ObjectStore {
        store: Arc<dyn ObjectStore>,
        prefix: String,
    },
}

impl SampleStore {
    /// Store a sample, returns the location identifier
    pub async fn put(&self, incident_id: Uuid, data: &[u8]) -> Result<String> {
        // Compress with zstd
        let compressed = zstd::encode_all(data, 3)?;

        match &self.mode {
            StorageMode::Postgres => {
                let sample_id = Uuid::new_v4();
                sqlx::query!(
                    r#"
                    INSERT INTO samples (id, incident_id, payload)
                    VALUES ($1, $2, $3)
                    "#,
                    sample_id,
                    incident_id,
                    compressed
                )
                .execute(&self.db)
                .await?;

                // Update incident with sample location
                sqlx::query!(
                    "UPDATE incidents SET sample_location = $1 WHERE id = $2",
                    format!("pg:{}", sample_id),
                    incident_id
                )
                .execute(&self.db)
                .await?;

                Ok(format!("pg:{}", sample_id))
            }
            StorageMode::ObjectStore { store, prefix } => {
                let sample_id = Uuid::new_v4();
                let path = object_store::path::Path::from(format!(
                    "{}/{}/{}.zst",
                    prefix, incident_id, sample_id
                ));

                store.put(&path, compressed.into()).await?;

                let location = format!("obj:{}", path);

                // Update incident with sample location
                sqlx::query!(
                    "UPDATE incidents SET sample_location = $1 WHERE id = $2",
                    &location,
                    incident_id
                )
                .execute(&self.db)
                .await?;

                Ok(location)
            }
        }
    }

    /// Retrieve a sample by location identifier
    pub async fn get(&self, location: &str) -> Result<Vec<u8>> {
        let compressed = if let Some(id_str) = location.strip_prefix("pg:") {
            let id: Uuid = id_str.parse()?;

            sqlx::query_scalar!(r#"SELECT payload FROM samples WHERE id = $1"#, id)
                .fetch_one(&self.db)
                .await?
        } else if let Some(path_str) = location.strip_prefix("obj:") {
            match &self.mode {
                StorageMode::ObjectStore { store, .. } => {
                    let path = object_store::path::Path::from(path_str);
                    let result = store.get(&path).await?;
                    result.bytes().await?.to_vec()
                }
                StorageMode::Postgres => {
                    anyhow::bail!(
                        "Object store location but running in postgres mode: {}",
                        location
                    )
                }
            }
        } else {
            anyhow::bail!("Invalid sample location: {}", location)
        };

        // Decompress
        let decompressed = zstd::decode_all(compressed.as_slice())?;
        Ok(decompressed)
    }

    /// Delete a sample by location identifier
    pub async fn delete(&self, location: &str) -> Result<()> {
        if let Some(id_str) = location.strip_prefix("pg:") {
            let id: Uuid = id_str.parse()?;

            sqlx::query!("DELETE FROM samples WHERE id = $1", id)
                .execute(&self.db)
                .await?;
        } else if let Some(path_str) = location.strip_prefix("obj:") {
            match &self.mode {
                StorageMode::ObjectStore { store, .. } => {
                    let path = object_store::path::Path::from(path_str);
                    store.delete(&path).await?;
                }
                StorageMode::Postgres => {
                    anyhow::bail!(
                        "Object store location but running in postgres mode: {}",
                        location
                    )
                }
            }
        } else {
            anyhow::bail!("Invalid sample location: {}", location)
        }

        Ok(())
    }

    /// List all samples for an incident
    pub async fn list_for_incident(&self, incident_id: Uuid) -> Result<Vec<String>> {
        match &self.mode {
            StorageMode::Postgres => {
                let samples = sqlx::query_scalar!(
                    "SELECT CONCAT('pg:', id::text) FROM samples WHERE incident_id = $1",
                    incident_id
                )
                .fetch_all(&self.db)
                .await?;

                Ok(samples.into_iter().flatten().collect())
            }
            StorageMode::ObjectStore { store, prefix } => {
                let path =
                    object_store::path::Path::from(format!("{}/{}/", prefix, incident_id));

                let mut locations = Vec::new();
                let mut list_stream = store.list(Some(&path));

                use futures::StreamExt;
                while let Some(meta) = list_stream.next().await {
                    if let Ok(meta) = meta {
                        locations.push(format!("obj:{}", meta.location));
                    }
                }

                Ok(locations)
            }
        }
    }
}

/// Initialize the sample store based on configuration
pub async fn init(config: &StorageConfig, db: PgPool) -> Result<SampleStore> {
    let mode = match config.mode.as_str() {
        "postgres" => {
            tracing::info!("Using Postgres for sample storage");
            StorageMode::Postgres
        }
        "object_store" => {
            let obj_config = config
                .object_store
                .as_ref()
                .ok_or_else(|| anyhow!("object_store config required when mode is object_store"))?;

            let store = build_object_store(obj_config).await?;
            let prefix = "crispr/samples".to_string();

            tracing::info!(
                provider = %obj_config.provider,
                bucket = %obj_config.bucket,
                "Using object store for sample storage"
            );

            StorageMode::ObjectStore {
                store: Arc::from(store),
                prefix,
            }
        }
        _ => anyhow::bail!("Invalid storage mode: {}", config.mode),
    };

    Ok(SampleStore { mode, db })
}

/// Build the appropriate object store based on provider
async fn build_object_store(config: &ObjectStoreConfig) -> Result<Box<dyn ObjectStore>> {
    match config.provider.as_str() {
        "s3" | "minio" | "r2" => {
            let mut builder = AmazonS3Builder::from_env().with_bucket_name(&config.bucket);

            if let Some(region) = &config.region {
                builder = builder.with_region(region);
            }

            if let Some(endpoint) = &config.endpoint {
                builder = builder.with_endpoint(endpoint);
                // For MinIO/R2, we need virtual-hosted-style requests disabled
                builder = builder.with_virtual_hosted_style_request(false);
            }

            // Allow HTTP for local MinIO
            if config.provider == "minio" {
                builder = builder.with_allow_http(true);
            }

            Ok(Box::new(builder.build()?))
        }
        "azure" => {
            use object_store::azure::MicrosoftAzureBuilder;

            let builder = MicrosoftAzureBuilder::from_env().with_container_name(&config.bucket);

            Ok(Box::new(builder.build()?))
        }
        "gcp" => {
            use object_store::gcp::GoogleCloudStorageBuilder;

            let builder = GoogleCloudStorageBuilder::from_env().with_bucket_name(&config.bucket);

            Ok(Box::new(builder.build()?))
        }
        _ => anyhow::bail!("Unsupported object store provider: {}", config.provider),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_parsing() {
        assert!("pg:123e4567-e89b-12d3-a456-426614174000"
            .strip_prefix("pg:")
            .is_some());
        assert!("obj:crispr/samples/abc/def.zst"
            .strip_prefix("obj:")
            .is_some());
    }
}
