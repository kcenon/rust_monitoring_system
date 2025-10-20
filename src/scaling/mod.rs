//! Auto-scaling and capacity management

pub mod autoscaler;

pub use autoscaler::{
    AutoScaler, AutoScalerConfig, PredictiveScaler, ScalingDecision, ScalingDirection, ScalingRule,
};
