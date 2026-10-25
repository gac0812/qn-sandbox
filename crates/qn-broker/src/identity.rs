//! Strict host capture metadata for pinned nono 0.80.0 (#103).
//! Schema: nono v0.80.0 `proxy_runtime.rs::capture_stdin_payload`; reverse-route
//! host/cache values were measured in #99. Authentication precedes this parser.
use serde::Deserialize;
use std::fmt;
const MAX_PAYLOAD: usize = 16 * 1024;
/// Rejections contain no request values, routing paths or credential material.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// Invalid host binding; never infer identity from a received request.
    #[error("invalid capture identity; bind a full native ID and managed route before launch")]
    Configuration,
    /// Request does not match the closed protocol and prebound identity.
    #[error("capture metadata denied; use the exact launch identity and configured route")]
    Denied,
    /// Parser failure with a redacted retained cause.
    #[error("capture metadata malformed; use the pinned provider schema: {0}")]
    Json(#[source] MetadataJsonError),
}
/// Retains parser location/category; hides attacker-controlled data in every formatter.
pub struct MetadataJsonError(serde_json::Error);
impl fmt::Display for MetadataJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at line {} column {}",
            self.0.classify(),
            self.0.line(),
            self.0.column()
        )
    }
}
impl fmt::Debug for MetadataJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for MetadataJsonError {}
/// Predetermined identity for one host-authenticated launch and reverse route.
/// This is not authentication, a grant, or proof that the owned session is live.
pub struct CaptureIdentity {
    native_id: String,
    reference: String,
}
impl fmt::Debug for CaptureIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureIdentity(..)")
    }
}
impl CaptureIdentity {
    /// Bind trusted launch metadata before accepting any requests; no learn-on-first-use.
    /// # Errors
    /// Requires 16 lowercase hex ID characters and a 1–64 character managed route.
    /// ```
    /// let identity = qn_broker::identity::CaptureIdentity::new("0123456789abcdef", "github")?;
    /// # Ok::<(), qn_broker::identity::IdentityError>(())
    /// ```
    pub fn new(native_id: &str, reference: &str) -> Result<Self, IdentityError> {
        if native_id.len() != 16
            || !native_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || reference.is_empty()
            || reference.len() > 64
            || !reference
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
        {
            return Err(IdentityError::Configuration);
        }
        Ok(Self {
            native_id: native_id.into(),
            reference: reference.into(),
        })
    }
    /// Parse bounded authenticated-channel input and match the complete fixed binding.
    /// Only method/path escape this boundary; origin/provider/grant remain host-owned.
    /// Caller must check live grant scope before provider IO and final admission.
    /// # Errors
    /// Rejects schema drift, unknown/duplicate fields, identity/routing mismatches,
    /// nonempty config/interaction, or unsupported method/path syntax.
    pub fn validate(&self, payload: &[u8]) -> Result<CaptureMetadata, IdentityError> {
        if payload.is_empty()
            || payload.len() > MAX_PAYLOAD
            || payload.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{')
        {
            return Err(IdentityError::Denied);
        }
        let input: WireRequest = serde_json::from_slice(payload)
            .map_err(|source| IdentityError::Json(MetadataJsonError(source)))?;
        if input.protocol != "nono.credential-provider.v1"
            || input.session_id != self.native_id
            || input.credential_name != self.reference
            || input.route_id != self.reference
            || input.request_host != self.reference
            || input.cache_scope != self.reference
            || input.request_method.is_empty()
            || input.request_method.len() > 16
            || !input.request_method.bytes().all(|b| b.is_ascii_uppercase())
            || !input.request_path.starts_with('/')
            || input.request_path.starts_with("//")
            || input.request_path.len() > 4096
            || input
                .request_path
                .chars()
                .any(|c| c.is_control() || matches!(c, '*' | '#' | '\\'))
        {
            return Err(IdentityError::Denied);
        }
        Ok(CaptureMetadata {
            method: input.request_method,
            path: input.request_path,
        })
    }
}
/// Validated exact method/path; still requires authorization against trusted origin.
pub struct CaptureMetadata {
    method: String,
    path: String,
}
impl CaptureMetadata {
    /// Exact uppercase request method; no normalization or wildcard expansion.
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }
    /// Exact path including query; never log request values or use them as commands.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}
impl fmt::Debug for CaptureMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureMetadata(..)")
    }
}
struct Empty {}
impl<'de> Deserialize<'de> for Empty {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields =
            std::collections::BTreeMap::<String, serde::de::IgnoredAny>::deserialize(deserializer)?;
        if fields.is_empty() {
            Ok(Self {})
        } else {
            Err(serde::de::Error::custom("expected an empty object"))
        }
    }
}
// Ten wire fields intentionally mirror the pinned upstream schema. No flatten/map
// parsing: serde must reject duplicate keys before they can overwrite a value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    protocol: String,
    session_id: String,
    credential_name: String,
    route_id: String,
    request_host: String,
    request_path: String,
    request_method: String,
    cache_scope: String,
    #[serde(rename = "config")]
    _config: Empty,
    #[serde(rename = "interaction")]
    _interaction: Empty,
}
#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
