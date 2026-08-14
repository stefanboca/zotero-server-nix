use super::*;

impl Oidc {
    pub fn begin_authorization(&self) -> anyhow::Result<OidcAuthorization> {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut authorization = self.client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        authorization = authorization
            .add_scope(Scope::new("profile".to_owned()))
            .add_scope(Scope::new("email".to_owned()));
        let (url, state, nonce) = authorization.set_pkce_challenge(challenge).url();
        Ok(OidcAuthorization {
            url: url.to_string(),
            state: state.secret().to_owned(),
            nonce: nonce.secret().to_owned(),
            pkce_verifier: verifier.secret().to_owned(),
        })
    }

    pub async fn exchange_code(
        &self,
        code: &str,
        flow: &OidcFlow,
    ) -> anyhow::Result<Option<OidcProfile>> {
        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_owned()))?
            .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier.clone()))
            .request_async(&self.http_client)
            .await
            .context("exchanging OIDC authorization code")?;
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow::anyhow!("OIDC provider omitted ID token"))?;
        let verifier = self.client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &Nonce::new(flow.nonce.clone()))
            .context("validating OIDC ID token")?;

        if let Some(expected_hash) = claims.access_token_hash() {
            let actual_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                id_token.signing_alg()?,
                id_token.signing_key(&verifier)?,
            )?;
            if &actual_hash != expected_hash {
                anyhow::bail!("OIDC access-token hash did not match ID token");
            }
        }

        let user_info: Option<CoreUserInfoClaims> = if self.client.user_info_url().is_some() {
            Some(
                self.client
                    .user_info(
                        token_response.access_token().clone(),
                        Some(claims.subject().clone()),
                    )?
                    .request_async(&self.http_client)
                    .await
                    .context("requesting OIDC UserInfo")?,
            )
        } else {
            None
        };
        let email = user_info
            .as_ref()
            .and_then(|claims| claims.email())
            .or_else(|| claims.email())
            .map(|email| email.as_str())
            .filter(|email| !email.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("OIDC provider did not supply an email address"))?;
        let email_verified = user_info
            .as_ref()
            .and_then(|claims| claims.email_verified())
            .or_else(|| claims.email_verified())
            .unwrap_or(false);
        if self.config.oidc.require_verified_email && !email_verified {
            return Ok(None);
        }
        Ok(Some(OidcProfile {
            issuer: claims.issuer().as_str().to_owned(),
            subject: claims.subject().as_str().to_owned(),
            email: email.to_owned(),
            username: user_info
                .as_ref()
                .and_then(|claims| claims.preferred_username())
                .or_else(|| claims.preferred_username())
                .map(|username| username.as_str().to_owned()),
            real_name: user_info
                .as_ref()
                .and_then(|claims| claims.name())
                .or_else(|| claims.name())
                .and_then(|name| name.get(None))
                .map(|name| name.as_str().to_owned()),
            email_verified,
        }))
    }
}
