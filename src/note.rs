use crate::envelope::ADSR;

#[derive(Clone)]
pub(crate) struct Note {
    pub(crate) frequency: f64,
    pub(crate) start_sec: f64,
    pub(crate) duration_sec: f64,
    pub(crate) amplitude: f64,
    pub(crate) env: ADSR,
}
