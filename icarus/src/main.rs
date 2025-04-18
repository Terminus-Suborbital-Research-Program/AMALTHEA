// Specifies that the standard library is not used
#![no_std]
#![no_main]

// Our Modules
pub mod actuators;
pub mod communications;
pub mod device_constants;
pub mod peripherals;
pub mod phases;
pub mod sensors;
pub mod startup;
pub mod tasks;
pub mod utilities;

use defmt_rtt as _; // global logger

// We require an allocator for some heap stuff - unfortunatly bincode serde
// doesn't have support for heapless vectors yet
extern crate alloc;

// Allocator
use linked_list_allocator::LockedHeap;

use crate::tasks::*;
use core::mem::MaybeUninit;
use icarus::{DelayTimer, I2CMainBus};

// Sensors
use bme280_rs::{AsyncBme280, Configuration, Oversampling, SensorMode};
use ina260_terminus::AsyncINA260;

// Busses
use rtic_sync::arbiter::i2c::ArbiterDevice;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
static mut HEAP_MEMORY: [u8; 1024 * 64] = [0; 1024 * 64];

/// Lets us know when we panic
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::error!("Panic: {}", info);
    loop {
        // Halt the CPU
        unsafe {
            hal::sio::spinlock_reset();
        }
    }
}

// HAL Access
#[cfg(feature = "rp2350")]
use rp235x_hal as hal;

// Monotonics
#[cfg(feature = "rp2350")]
use rtic_monotonics::rp235x::prelude::*;
#[cfg(feature = "rp2350")]
rp235x_timer_monotonic!(Mono);

/// Tell the Boot ROM about our application
#[link_section = ".start_block"]
#[used]
#[cfg(feature = "rp2350")]
pub static IMAGE_DEF: rp235x_hal::block::ImageDef = rp235x_hal::block::ImageDef::secure_exe();

#[rtic::app(
    device = hal::pac,
    dispatchers = [PIO2_IRQ_0, PIO2_IRQ_1, DMA_IRQ_0],
    peripherals = true,
)]
mod app {
    use crate::{
        actuators::servo::{EjectionServo, LockingServo},
        communications::{
            hc12::{UART1Bus, GPIO10},
            link_layer::LinkLayerDevice,
        },
        device_constants::{AvionicsI2cBus, IcarusStateMachine, MotorI2cBus, ReactionWheelMotor},
        phases::StateMachineListener,
    };

    use super::*;

    use bin_packets::IcarusPhase;
    use communications::*;

    use cortex_m::delay::Delay;
    use embedded_hal_async::delay::DelayNs;
    use hal::gpio::{self, FunctionSio, PullNone, SioOutput};
    use rp235x_hal::{timer::CopyableTimer1, uart::UartPeripheral};
    pub const XTAL_FREQ_HZ: u32 = 12_000_000u32;

    use rtic_sync::{
        arbiter::Arbiter,
        signal::{Signal, SignalReader},
    };
    use usb_device::class_prelude::*;

    use hc12::HC12;

    // use usbd_serial::SerialPort;

    pub type UART0Bus = UartPeripheral<
        rp235x_hal::uart::Enabled,
        rp235x_hal::pac::UART0,
        (
            gpio::Pin<gpio::bank0::Gpio0, gpio::FunctionUart, gpio::PullDown>,
            gpio::Pin<gpio::bank0::Gpio1, gpio::FunctionUart, gpio::PullDown>,
        ),
    >;

    // pub static mut USB_BUS: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;
    #[shared]
    pub struct Shared {
        //uart0: UART0Bus,
        //uart0_buffer: heapless::String<HEAPLESS_STRING_ALLOC_LENGTH>,
        pub ejector_driver: EjectionServo,
        pub locking_driver: LockingServo,
        pub radio_link: LinkLayerDevice<HC12<UART1Bus, GPIO10>>,
        // pub usb_serial: SerialPort<'static, hal::usb::UsbBus>,
        pub clock_freq_hz: u32,
        pub state_machine: IcarusStateMachine,
    }

    #[local]
    pub struct Local {
        pub led: gpio::Pin<gpio::bank0::Gpio25, FunctionSio<SioOutput>, PullNone>,
        pub bme280: AsyncBme280<ArbiterDevice<'static, AvionicsI2cBus>, Mono>,
        pub ina260_1: AsyncINA260<ArbiterDevice<'static, MotorI2cBus>, Mono>,
        pub ina260_2: AsyncINA260<ArbiterDevice<'static, MotorI2cBus>, Mono>,
        pub ina260_3: AsyncINA260<ArbiterDevice<'static, MotorI2cBus>, Mono>,
    }

    #[init(
        local=[
            // Task local initialized resources are static
            // Here we use MaybeUninit to allow for initialization in init()
            // This enables its usage in driver initialization
            i2c_avionics_bus: MaybeUninit<Arbiter<AvionicsI2cBus>> = MaybeUninit::uninit(),
            i2c_motor_bus: MaybeUninit<Arbiter<MotorI2cBus>> = MaybeUninit::uninit(),
            esc_state_signal: MaybeUninit<Signal<IcarusPhase>> = MaybeUninit::uninit(),
        ]
    )]
    fn init(ctx: init::Context) -> (Shared, Local) {
        startup::startup(ctx)
    }

    extern "Rust" {
        // Heartbeats the main led
        #[task(local = [led], priority = 2)]
        async fn heartbeat(ctx: heartbeat::Context);

        // Takes care of incoming packets
        #[task(shared = [radio_link], priority = 1)]
        async fn incoming_packet_handler(mut ctx: incoming_packet_handler::Context);

        // Handler for the I2C electronic speed controllers
        #[task(priority = 3, shared = [state_machine])]
        async fn motor_drivers(
            &mut ctx: motor_drivers::Context,
            i2c: &'static Arbiter<MotorI2cBus>,
            mut esc_state_listener: StateMachineListener,
        );

        // Updates the radio module on the serial interrupt
        #[task(binds = UART1_IRQ, shared = [radio_link])]
        fn uart_interrupt(mut ctx: uart_interrupt::Context);

        // Radio Flush Task
        #[task(shared = [radio_link], priority = 1)]
        async fn radio_flush(mut ctx: radio_flush::Context);

        #[task(local = [bme280], priority = 3)]
        async fn sample_sensors(
            mut ctx: sample_sensors::Context,
            avionics_i2c: &'static Arbiter<AvionicsI2cBus>,
        );

        #[task(priority = 3)]
        async fn inertial_nav(mut ctx: inertial_nav::Context);
    }
}
