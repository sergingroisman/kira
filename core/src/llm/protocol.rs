//! Wire-protocol families supported by the Kira router.
//!
//! Each variant maps to one HTTP wire format.  v0 ships two:
//!
//! | Variant | Wire format | Providers |
//! |---|---|---|
//! | `OpenAiCompatible` | `/v1/chat/completions` + SSE | DashScope, DeepSeek, OpenAI, OpenRouter, Groq, Ollama, GitHub Copilot, SiliconFlow |
//! | `Anthropic` | `/v1/messages` + SSE | Anthropic Claude |

/// Identifies the wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI-compatible chat completions API.
    /// Every provider listed above uses this variant.
    OpenAiCompatible,
    /// Anthropic Messages API (`/v1/messages`, `x-api-key` header, content blocks).
    Anthropic,
}

impl Protocol {
    /// Parse from a config string.
    /// - `"openai-compatible"` → `OpenAiCompatible`
    /// - `"anthropic"` → `Anthropic`
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "openai-compatible" => Some(Self::OpenAiCompatible),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai-compatible",
            Self::Anthropic => "anthropic",
        }
    }
}
