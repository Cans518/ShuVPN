use super::{clear_display, model::SignInState, now, Account, AuthFailure, LoginResult};

pub(super) fn validate_credentials(username: &str, password: &str) -> Result<(), AuthFailure> {
    if username.is_empty() || username.len() > 256 || password.is_empty() {
        return Err(AuthFailure::new("credentials", "请输入校园账号和密码"));
    }
    Ok(())
}

impl Account {
    pub(crate) async fn sign_in_password(
        &self,
        username: String,
        password: String,
    ) -> Result<SignInState, AuthFailure> {
        validate_credentials(&username, &password)?;
        self.run("正在登录统一身份认证", async {
            let identity = self.login_identity(username, password).await?;
            if !identity.methods.is_empty() {
                return Ok(SignInState::VerificationRequired);
            }
            self.acquire_sign_in_credentials().await?;
            Ok(SignInState::Completed)
        })
        .await
    }

    pub(crate) async fn sign_in_code(
        &self,
        method: String,
        code: String,
    ) -> Result<(), AuthFailure> {
        self.run("正在校验验证码", async {
            self.verify_identity(method, code).await?;
            self.acquire_sign_in_credentials().await
        })
        .await
    }

    async fn acquire_sign_in_credentials(&self) -> Result<(), AuthFailure> {
        if self.exchange_all().await? {
            Ok(())
        } else {
            Err(AuthFailure::new(
                "credentialsUnavailable",
                "凭据交换失败，请重新登录",
            ))
        }
    }

    pub(super) async fn login_identity(
        &self,
        username: String,
        password: String,
    ) -> Result<LoginResult, AuthFailure> {
        clear_display(&mut *self.active()?);
        let result = self.client.login(username, password).await?;
        if result.methods.is_empty() {
            if !self.client.snapshot()?.has_session {
                return Err(AuthFailure::new(
                    "sessionMissing",
                    "统一认证没有建立账号会话",
                ));
            }
            self.active()?.signed_in_at = Some(now());
        }
        Ok(result)
    }

    pub(super) async fn verify_identity(
        &self,
        method: String,
        code: String,
    ) -> Result<LoginResult, AuthFailure> {
        let result = self.client.verify_code(method, code).await?;
        if !self.client.snapshot()?.has_session {
            return Err(AuthFailure::new(
                "sessionMissing",
                "验证码通过后没有建立账号会话",
            ));
        }
        self.active()?.signed_in_at = Some(now());
        Ok(result)
    }
}
