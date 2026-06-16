use proximate_driver::Error;

mod backend;
mod consts;
mod device;
mod driver;
#[cfg(test)]
mod fake;
mod runtime;
mod target;
#[cfg(test)]
mod tests;

pub(super) use driver::Pn71xxDriver;

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}
