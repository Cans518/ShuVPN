use super::{cancelled_failure, live, model::JwxtAccount, Client, Transaction};
use crate::auth::model::AuthFailure;
use std::time::Duration;
mod parse;

impl Client {
    /// Optional display data; callers authorize JWXT before reading.
    pub(crate) async fn fetch_jwxt_account(&self) -> Result<JwxtAccount, AuthFailure> {
        let (generation, cancelled) = {
            let state = self.state()?;
            live(&state)?;
            (state.generation, state.cancelled.clone())
        };
        tokio::select! {
            biased;
            _ = cancelled.cancelled() => Err(cancelled_failure()),
            result = async {
                let _operation = self.operation.lock().await;
                let mut transaction = self.begin(generation)?;
                self.require_session(&transaction)?;
                let profile_jar = transaction.jar.clone();
                let mut account = match tokio::time::timeout(Duration::from_secs(15), self.jwxt_profile(&mut transaction))
                    .await.ok().and_then(Result::ok).flatten() {
                    Some(account) => account,
                    None => {
                        transaction.jar = profile_jar;
                        JwxtAccount::default()
                    }
                };
                let identity_jar = transaction.jar.clone();
                if let Some(identity) = tokio::time::timeout(Duration::from_secs(30), self.jwxt_identity(&mut transaction))
                    .await.ok().and_then(Result::ok).flatten() {
                    account.name = identity.name.or(account.name);
                    account.student_id = identity.student_id.or(account.student_id);
                    account.class_name = identity.class_name;
                } else {
                    transaction.jar = identity_jar;
                }
                self.finish(transaction, Ok(account))
            } => result,
        }
    }

    async fn jwxt_profile(
        &self,
        transaction: &mut Transaction,
    ) -> Result<Option<JwxtAccount>, AuthFailure> {
        let base = self.config.system_base("jwxt")?;
        let mut uri = base
            .join("/jwglxt/xtgl/index_cxYhxxIndex.html")
            .map_err(|_| invalid())?;
        uri.query_pairs_mut()
            .append_pair("xt", "jw")
            .append_pair("localeKey", "zh_CN")
            .append_pair("gnmkdm", "index")
            .append_pair("_", &crate::auth::atrust::random_hex(8)?);
        let response = self
            .config
            .page(
                &mut transaction.jar,
                uri,
                &base
                    .join("/jwglxt/xtgl/index_initMenu.html")
                    .map_err(|_| invalid())?,
            )
            .await?;
        Ok((response.status.as_u16() == 200)
            .then(|| parse::profile(&response.body))
            .flatten())
    }

    async fn jwxt_identity(
        &self,
        transaction: &mut Transaction,
    ) -> Result<Option<JwxtAccount>, AuthFailure> {
        let base = self.config.system_base("jwxt")?;
        let index = base
            .join("/jwglxt/kbcx/xskbcx_cxXskbcxIndex.html?gnmkdm=N2151&layout=default")
            .map_err(|_| invalid())?;
        let response = self
            .config
            .page(
                &mut transaction.jar,
                index.clone(),
                &base
                    .join("/jwglxt/xtgl/index_initMenu.html")
                    .map_err(|_| invalid())?,
            )
            .await?;
        if response.status.as_u16() != 200 {
            return Ok(None);
        }
        let Some((year, term)) = parse::term(&response.body) else {
            return Ok(None);
        };
        let uri = base
            .join("/jwglxt/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151")
            .map_err(|_| invalid())?;
        let fields = [
            ("xnm", year),
            ("xqm", term),
            ("kzlx", "ck".into()),
            ("xsdm", String::new()),
            ("kclbdm", String::new()),
            ("kclxdm", String::new()),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
        let response = self
            .config
            .form(&mut transaction.jar, uri, &index, fields)
            .await?;
        Ok((response.status.as_u16() == 200)
            .then(|| parse::identity(&response.body))
            .flatten())
    }
}

fn invalid() -> AuthFailure {
    AuthFailure::new("jwxtResponse", "教务资料地址无效")
}
