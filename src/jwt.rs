use crate::auth;
use tracing::trace;

type DateTimeUtc = chrono::DateTime<chrono::Utc>;

type Claims = biscuit::ClaimsSet<auth::Claims>;

#[derive(Clone, Debug)]
pub struct Jwt {
    secret: String,
}

impl Jwt {
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_owned(),
        }
    }

    pub fn encode(&self, claims: auth::Claims, expiry: DateTimeUtc) -> anyhow::Result<String> {
        let registered = biscuit::RegisteredClaims {
            expiry: Some(biscuit::Timestamp::from(expiry)),
            ..Default::default()
        };

        let private = claims;
        let claims = Claims {
            registered,
            private,
        };
        trace!("Generating token with claims {:?}", claims);

        let jwt = biscuit::JWT::new_decoded(
            From::from(biscuit::jws::RegisteredHeader {
                algorithm: biscuit::jwa::SignatureAlgorithm::HS256,
                ..Default::default()
            }),
            claims,
        );

        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);

        jwt.into_encoded(&secret)
            .map(|t| t.unwrap_encoded().to_string())
            .map_err(|e| e.into())
    }

    pub fn decode(&self, token: &str) -> anyhow::Result<biscuit::ClaimsSet<auth::Claims>> {
        let token = biscuit::JWT::<auth::Claims, biscuit::Empty>::new_encoded(&token);
        let secret = biscuit::jws::Secret::bytes_from_str(&self.secret);
        let token = token.into_decoded(&secret, biscuit::jwa::SignatureAlgorithm::HS256)?;
        let claims = token.payload()?;
        claims
            .registered
            .validate_exp(biscuit::Validation::Validate(biscuit::TemporalOptions {
                epsilon: chrono::Duration::seconds(2),
                now: None,
            }))?;
        Ok(claims.to_owned())
    }
}
