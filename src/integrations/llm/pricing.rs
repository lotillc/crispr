//! LLM pricing calculation
//!
//! Loads pricing from config/pricing.toml and calculates costs per request.
//! Prices are per million tokens (MTok).

use anyhow::Result;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

/// Pricing configuration loaded from TOML
#[derive(Debug, Clone)]
pub struct PricingConfig {
    pub anthropic: HashMap<String, ModelPricing>,
    pub openai: HashMap<String, ModelPricing>,
    pub local: HashMap<String, ModelPricing>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    pub input: f64,  // Price per million tokens
    pub output: f64, // Price per million tokens
}

impl PricingConfig {
    /// Calculate cost for a request
    pub fn calculate_cost(
        &self,
        provider: &str,
        model: &str,
        input_tokens: i32,
        output_tokens: i32,
    ) -> Decimal {
        let pricing = match provider {
            "anthropic" => self.anthropic.get(model),
            "openai" => self.openai.get(model),
            "local" | "ollama" => {
                // Local models are free
                return Decimal::ZERO;
            }
            _ => None,
        };

        match pricing {
            Some(p) => {
                let input_cost = (input_tokens as f64 / 1_000_000.0) * p.input;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * p.output;
                let total = input_cost + output_cost;

                // Convert to Decimal with 6 decimal places
                Decimal::try_from(total).unwrap_or(Decimal::ZERO)
            }
            None => {
                // Unknown model - estimate based on provider
                let (input_rate, output_rate) = match provider {
                    "anthropic" => (3.0, 15.0), // Assume Sonnet pricing
                    "openai" => (2.5, 10.0),    // Assume GPT-4o pricing
                    _ => (0.0, 0.0),
                };

                let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
                let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;
                let total = input_cost + output_cost;

                Decimal::try_from(total).unwrap_or(Decimal::ZERO)
            }
        }
    }
}

/// Load pricing from config/pricing.toml
pub fn load_pricing() -> Result<PricingConfig> {
    let pricing_path = std::env::var("CRISPR_PRICING")
        .unwrap_or_else(|_| "config/pricing.toml".to_string());

    let content = match std::fs::read_to_string(&pricing_path) {
        Ok(c) => c,
        Err(_) => {
            // Use default pricing if file not found
            tracing::warn!(path = %pricing_path, "Pricing file not found, using defaults");
            return Ok(default_pricing());
        }
    };

    let raw: RawPricingConfig = toml::from_str(&content)?;

    Ok(PricingConfig {
        anthropic: raw.anthropic.unwrap_or_default(),
        openai: raw.openai.unwrap_or_default(),
        local: raw.local.unwrap_or_default(),
    })
}

/// Default pricing if config file is missing
fn default_pricing() -> PricingConfig {
    let mut anthropic = HashMap::new();
    anthropic.insert(
        "claude-3-haiku-20240307".to_string(),
        ModelPricing {
            input: 0.25,
            output: 1.25,
        },
    );
    anthropic.insert(
        "claude-3-5-haiku-20241022".to_string(),
        ModelPricing {
            input: 0.80,
            output: 4.00,
        },
    );
    anthropic.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelPricing {
            input: 3.00,
            output: 15.00,
        },
    );
    anthropic.insert(
        "claude-sonnet-4-20250514".to_string(),
        ModelPricing {
            input: 3.00,
            output: 15.00,
        },
    );
    anthropic.insert(
        "claude-opus-4-20250514".to_string(),
        ModelPricing {
            input: 15.00,
            output: 75.00,
        },
    );

    let mut openai = HashMap::new();
    openai.insert(
        "gpt-4o-mini".to_string(),
        ModelPricing {
            input: 0.15,
            output: 0.60,
        },
    );
    openai.insert(
        "gpt-4o".to_string(),
        ModelPricing {
            input: 2.50,
            output: 10.00,
        },
    );
    openai.insert(
        "gpt-4-turbo".to_string(),
        ModelPricing {
            input: 10.00,
            output: 30.00,
        },
    );
    openai.insert(
        "o1".to_string(),
        ModelPricing {
            input: 15.00,
            output: 60.00,
        },
    );
    openai.insert(
        "o1-mini".to_string(),
        ModelPricing {
            input: 3.00,
            output: 12.00,
        },
    );

    let local = HashMap::new();

    PricingConfig {
        anthropic,
        openai,
        local,
    }
}

// Raw TOML structure
#[derive(Deserialize)]
struct RawPricingConfig {
    anthropic: Option<HashMap<String, ModelPricing>>,
    openai: Option<HashMap<String, ModelPricing>>,
    local: Option<HashMap<String, ModelPricing>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost() {
        let pricing = default_pricing();

        // Test Anthropic Haiku
        let cost = pricing.calculate_cost("anthropic", "claude-3-haiku-20240307", 1000, 500);
        assert!(cost > Decimal::ZERO);

        // Test local (should be free)
        let cost = pricing.calculate_cost("local", "llama3", 1000, 500);
        assert_eq!(cost, Decimal::ZERO);
    }

    #[test]
    fn test_default_pricing() {
        let pricing = default_pricing();
        assert!(!pricing.anthropic.is_empty());
        assert!(!pricing.openai.is_empty());
    }
}
