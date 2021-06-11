#![allow(dead_code)]

mod vmcommand;
mod vmemulator;
mod vmparser;

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, ImageData};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
pub use vmcommand::{
    Command as VMCommand, Operation as VMOperation, Segment as VMSegment, VMProgram,
};
pub use vmemulator::VMEmulator;

#[wasm_bindgen]
pub fn init_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    //   #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    // The `console.log` is quite polymorphic, so we can bind it with multiple
    // signatures. Note that we need to use `js_name` to ensure we always call
    // `log` in JS.
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    // Multiple arguments too!
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

macro_rules! console_log {
    // Note that this is using the `log` function imported above during
    // `bare_bones`
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct WebVM {
    vm: VMEmulator,
    files: Vec<(String, String)>,
}

#[wasm_bindgen]
impl WebVM {
    pub fn new() -> WebVM {
        WebVM {
            vm: VMEmulator::empty(),
            files: Vec::new(),
        }
    }

    pub fn load_file(&mut self, name: &str, content: &str) {
        self.files.push((name.to_string(), content.to_string()));
    }

    pub fn init(&mut self) -> Result<(), JsValue> {
        let files: Vec<(&str, &str)> = self.files.iter().map(|(a, b)| (&a[..], &b[..])).collect();
        let program = VMProgram::with_internals(&files, Some(VMEmulator::get_internals()))
            .map_err(|e| format!("Failed to parse program: {}", e))?;
        for warning in program.warnings.iter() {
            console_log!("Warning: {}", warning);
        }
        let mut vm = VMEmulator::new(program);
        vm.init()
            .map_err(|e| format!("Failed to initialize program: {}", e))?;
        self.vm = vm;
        Ok(())
    }

    pub fn tick(&mut self, n: i32) -> Result<(), JsValue> {
        for _ in 0..n {
            match self.vm.step() {
                Err(e) => return Err(JsValue::from(e)),
                Ok(_) => {}
            };
        }
        Ok(())
    }

    pub fn tick_profiled(&mut self, n: i32) -> Result<(), JsValue> {
        for _ in 0..n {
            self.vm.profile_step();
            match self.vm.step() {
                Err(e) => return Err(JsValue::from(e)),
                Ok(_) => {}
            };
        }
        Ok(())
    }

    pub fn set_keyboard(&mut self, key: u16) -> Result<(), JsValue> {
        match self.vm.set_ram(24576, key as i32) {
            Err(e) => Err(JsValue::from(e)),
            Ok(_) => Ok(()),
        }
    }

    pub fn get_ram(&mut self, address: usize) -> Option<i32> {
        self.vm.ram().get(address).copied()
    }

    pub fn reset(&mut self) {
        self.vm.reset();
    }

    pub fn draw_screen(&mut self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        let start = 16384;
        let end = start + 512 * 256 / 16;
        let ram = self.vm.get_ram_range(start, end);
        draw_from_ram(ram, ctx)
    }

    pub fn get_stats(&self) -> JsValue {
        JsValue::from(format!("Stats: \n{}", self.vm.profiler_stats()))
    }

    pub fn get_debug(&self) -> JsValue {
        JsValue::from(self.vm.debug())
    }
}

fn pixels_from_ram(ram: &[i32]) -> Vec<u8> {
    let mut data = Vec::new();
    for a in ram.iter() {
        if *a == 0 {
            for _ in 0..16 * 4 {
                data.push(0xff); // white
            }
        } else {
            for i in 0..16 {
                if a & 1 << i > 0 {
                    data.push(0x00);
                    data.push(0x00);
                    data.push(0x00);
                    data.push(0xff);
                } else {
                    data.push(0xff);
                    data.push(0xff);
                    data.push(0xff);
                    data.push(0xff);
                }
            }
        }
    }
    data
}

fn draw_from_ram(ram: &[i32], ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
    let mut data = pixels_from_ram(ram);

    let width = 512;
    let height = 256;

    let data = ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&mut data),
        width,
        height,
    )?;
    ctx.put_image_data(&data, 0.0, 0.0)
}
