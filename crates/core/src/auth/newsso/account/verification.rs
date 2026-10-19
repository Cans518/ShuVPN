use super::super::model::VerificationTarget;
use crate::auth::model::AuthFailure;
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;

pub struct VerificationState {
    pub methods: Vec<VerificationTarget>,
    pub selected_method: String,
    pub resend_after_seconds: u32,
}

#[derive(Default)]
pub(super) struct Delivery {
    selected: Option<String>,
    deadlines: BTreeMap<String, Instant>,
}

impl Delivery {
    pub(super) fn snapshot(&self, methods: Vec<VerificationTarget>) -> Option<VerificationState> {
        let selected = self
            .selected
            .as_ref()
            .and_then(|selected| methods.iter().find(|target| target.method == *selected))
            .or_else(|| methods.iter().find(|target| target.method == "wecom"))
            .or_else(|| methods.first())?;
        Some(VerificationState {
            selected_method: selected.method.clone(),
            resend_after_seconds: self.remaining(&selected.method),
            methods,
        })
    }

    fn remaining(&self, method: &str) -> u32 {
        self.deadlines
            .get(method)
            .map(|deadline| {
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_nanos()
                    .div_ceil(1_000_000_000) as u32
            })
            .unwrap_or(0)
    }

    pub(super) fn select(
        &mut self,
        methods: &[VerificationTarget],
        method: String,
    ) -> Result<(), AuthFailure> {
        if !methods.iter().any(|target| target.method == method) {
            return Err(AuthFailure::new("invalidMethod", "当前验证方式不可用"));
        }
        self.selected = Some(method);
        Ok(())
    }

    pub(super) fn begin(
        &mut self,
        methods: &[VerificationTarget],
        method: &str,
    ) -> Result<(), AuthFailure> {
        self.select(methods, method.to_owned())?;
        if self.remaining(method) > 0 {
            return Err(AuthFailure::new(
                "sendCooldown",
                "请等待验证码重发倒计时结束",
            ));
        }
        // A cancelled request may already have reached the provider. Keep the
        // reservation across page disposal, login attempts and method changes.
        self.deadlines
            .insert(method.to_owned(), Instant::now() + Duration::from_secs(60));
        Ok(())
    }

    pub(super) fn failed(&mut self, methods: &[VerificationTarget], method: &str, code: &str) {
        if code.eq_ignore_ascii_case("senderror") {
            if let Some(alternate) = methods.iter().find(|target| target.method != method) {
                self.selected = Some(alternate.method.clone());
            }
        }
    }

    pub(super) fn reset_selection(&mut self) {
        self.selected = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn methods() -> Vec<VerificationTarget> {
        ["sms", "wecom"]
            .into_iter()
            .map(|method| VerificationTarget {
                method: method.into(),
                target: "masked".into(),
            })
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn cooldown_survives_selection_reset_and_expires_at_the_native_deadline(
    ) -> anyhow::Result<()> {
        let mut delivery = Delivery::default();
        let methods = methods();
        assert_eq!(
            delivery
                .snapshot(methods.clone())
                .ok_or_else(|| anyhow::anyhow!("missing challenge"))?
                .selected_method,
            "wecom"
        );
        delivery.begin(&methods, "wecom")?;
        delivery.select(&methods, "sms".into())?;
        delivery.reset_selection();
        assert_eq!(delivery.remaining("wecom"), 60);
        assert!(
            matches!(delivery.begin(&methods, "wecom"), Err(error) if error.code == "sendCooldown")
        );
        tokio::time::advance(Duration::from_millis(59_999)).await;
        assert_eq!(delivery.remaining("wecom"), 1);
        tokio::time::advance(Duration::from_millis(1)).await;
        delivery.begin(&methods, "wecom")?;
        assert_eq!(delivery.remaining("wecom"), 60);
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn rejection_selects_an_alternate_without_sending_or_clearing_reservations(
    ) -> anyhow::Result<()> {
        let mut delivery = Delivery::default();
        let methods = methods();
        assert!(delivery.begin(&methods, "unknown").is_err());
        assert!(delivery.deadlines.is_empty());
        delivery.begin(&methods, "wecom")?;
        delivery.failed(&methods, "wecom", "network");
        assert_eq!(delivery.selected.as_deref(), Some("wecom"));
        delivery.failed(&methods, "wecom", "SendError");
        assert_eq!(delivery.selected.as_deref(), Some("sms"));
        assert_eq!(delivery.remaining("sms"), 0);
        assert_eq!(delivery.remaining("wecom"), 60);
        assert!(delivery.snapshot(vec![]).is_none());
        let sms = vec![methods[0].clone()];
        delivery.begin(&sms, "sms")?;
        delivery.failed(&sms, "sms", "senderror");
        assert_eq!(delivery.selected.as_deref(), Some("sms"));
        Ok(())
    }
}
