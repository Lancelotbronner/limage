use crate::LimageConfig;
use ovmf_prebuilt::Prebuilt;

pub struct Kernel {
    pub config: LimageConfig,
    pub prebuilt: Prebuilt,
}
