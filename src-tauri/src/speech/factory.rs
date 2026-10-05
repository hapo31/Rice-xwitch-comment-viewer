use super::bouyomi::{classify_error, BouyomiAdapter, BouyomiError, BouyomiTalkConfig};
use super::runtime::{SpeechAdapterFactory, SpeechDispatcher};
use super::{SpeechAdapter, SpeechFailure};
use crate::settings::{SpeechAdapterKind, SpeechSettings};
use std::sync::Arc;

pub struct ConfiguredAdapterFactory {
    pub(crate) policy: Arc<super::destination::DestinationPolicy>,
}

impl SpeechAdapterFactory for ConfiguredAdapterFactory {
    fn select(
        &self,
        settings: &SpeechSettings,
        dispatcher: SpeechDispatcher,
    ) -> Result<Arc<dyn SpeechAdapter>, SpeechFailure> {
        match settings.adapter {
            SpeechAdapterKind::Bouyomi => Ok(Arc::new(bouyomi_from_settings(
                settings,
                dispatcher,
                self.policy.clone(),
            )?)),
        }
    }
    fn connection_confirmation(&self, settings: &SpeechSettings) -> Option<String> {
        match settings.adapter {
            SpeechAdapterKind::Bouyomi => settings.connection_success_speech_enabled.then(|| {
                super::bouyomi::normalize_connection_success_message(
                    &settings.connection_success_speech_text,
                )
                .to_string()
            }),
        }
    }
}

/// Concrete settings and construction are outside queue/worker/commands.
pub(crate) fn bouyomi_from_settings(
    settings: &SpeechSettings,
    dispatcher: SpeechDispatcher,
    policy: Arc<super::destination::DestinationPolicy>,
) -> Result<BouyomiAdapter, SpeechFailure> {
    crate::settings::validation::validate_speech(settings)
        .map_err(|error| classify_error(BouyomiError::Destination(error).into()))?;
    BouyomiAdapter::with_dispatcher(
        &settings.bouyomi_host,
        settings.bouyomi_port,
        BouyomiTalkConfig {
            speed: settings.bouyomi_speed,
            tone: settings.bouyomi_tone,
            volume: settings.bouyomi_volume,
            voice: settings.bouyomi_voice,
            code: 0,
        },
        dispatcher,
    )
    .map(|adapter| adapter.with_destination_policy(policy, settings.bouyomi_remote_mode))
    .map_err(|detail| classify_error(BouyomiError::Configuration(detail).into()))
}
