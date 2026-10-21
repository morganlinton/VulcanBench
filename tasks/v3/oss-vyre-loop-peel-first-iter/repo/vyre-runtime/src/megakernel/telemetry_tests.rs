// Tests for `telemetry.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.

use super::*;
use crate::megakernel::descriptor::WindowClass;
use crate::megakernel::protocol::{opcode, SLOT_WORDS};
use crate::megakernel::Megakernel;
use crate::megakernel::{
    MegakernelDispatchTopology, MegakernelExecutionMode, MegakernelLaunchRequest,
};

#[path = "telemetry_tests/decode_contracts.rs"]
mod decode_contracts;
#[path = "telemetry_tests/window_contracts.rs"]
mod window_contracts;
#[path = "telemetry_tests/recommendation_runtime_contracts.rs"]
mod recommendation_runtime_contracts;
#[path = "telemetry_tests/sketch_watchdog_contracts.rs"]
mod sketch_watchdog_contracts;
