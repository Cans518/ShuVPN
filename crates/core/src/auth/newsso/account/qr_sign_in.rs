use super::{
    cancelled_error, clear_display,
    model::{QrSignInEvent, QrSignInPhase},
    now, Account, AuthFailure, QrStatus,
};

impl Account {
    pub(crate) async fn sign_in_qr(
        &self,
        emit: impl FnMut(QrSignInEvent) -> bool + Send,
    ) -> Result<bool, AuthFailure> {
        let emitter = std::sync::Mutex::new(emit);
        let emit = |event| emitter.lock().is_ok_and(|mut emit| emit(event));
        self.run_notifying(
            "正在创建二维码",
            async {
                {
                    let mut view = self.active()?;
                    clear_display(&mut view);
                    view.qr_active = true;
                    view.qr_flow = true;
                }
                let image = self.client.start_qr().await?;
                self.active()?.progress = Some("等待扫码确认".into());
                if !emit(QrSignInEvent {
                    phase: QrSignInPhase::Waiting,
                    image: Some(image),
                    error: None,
                }) {
                    self.cancel_qr()?;
                    return Err(cancelled_error());
                }
                if self.client.wait_qr(180).await? != QrStatus::Succeeded {
                    return Err(AuthFailure::new(
                        "qrExpired",
                        "二维码已过期或已取消，请重新尝试",
                    ));
                }
                self.active()?.progress = Some("正在接管企业微信会话".into());
                if !emit(QrSignInEvent {
                    phase: QrSignInPhase::Redeeming,
                    image: None,
                    error: None,
                }) {
                    self.cancel_qr()?;
                    return Err(cancelled_error());
                }
                self.client.redeem_qr().await?;
                {
                    let mut view = self.active()?;
                    view.signed_in_at = Some(now());
                    view.qr_active = false;
                    view.progress = Some("正在获取凭据".into());
                }
                if !emit(QrSignInEvent {
                    phase: QrSignInPhase::Credentials,
                    image: None,
                    error: None,
                }) {
                    self.cancel_qr()?;
                    return Err(cancelled_error());
                }
                self.exchange_all().await
            },
            |result| {
                if let Ok(available) = result {
                    let _ = emit(QrSignInEvent {
                        phase: if *available {
                            QrSignInPhase::Completed
                        } else {
                            QrSignInPhase::Unavailable
                        },
                        image: None,
                        error: None,
                    });
                }
            },
        )
        .await
    }
}
