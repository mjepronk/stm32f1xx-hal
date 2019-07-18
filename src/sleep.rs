/*!
 * TODO:
 * - What to do when we do not have the NVIC? (RTFM)
 * - Support for USB and Ethernet wakeup events
 * - Separate function for slowing down system clocks?
 * - Separate function for SLEEPONEXIT?
 * - Separate function to put GPIO's in analog input mode?
 */

use cortex_m::asm;
use cortex_m::peripheral::SCB;
use cortex_m::peripheral::NVIC;
use crate::{
    pac::{EXTI, PWR, DBGMCU},
    prelude::*,
    rtc::Rtc,
    rcc::{Rcc, APB1},
    pac::Interrupt::{EXTI0, EXTI3, RTC, RTCALARM},
};

/// Sleep modes in descending order of power usage and in ascending or order of
/// wakeup time.
#[derive(PartialEq)]
pub enum SleepMode {
    Sleep,            // CPU clock off (default for wfi or wfe)
    StopRegulatorOn,  // All 1.8V domain clocks off, voltage regulator ON
    StopRegulatorLP,  // All 1.8V domain clocks off, voltage regulator in low power mode
    Standby,          // All 1.8V domain clocks off, voltage regulator OFF
}

pub enum SleepModeEntry {
    WFI, // Wait for Interrupt
    WFE, // Wait for Event, offers the lowest wakeup time
}

/// This function initializes most sleep related registers and enters sleep mode
/// using either the WFI or WFE instruction. For more information see section
/// "5.3 Low-power modes" of the RM0008 reference manual.
pub fn enter_sleep_mode(
    sleep_mode: SleepMode,
    sleep_mode_entry: SleepModeEntry,
    enable_wakeup_alarm: bool,
    enable_wakeup_pin: bool,
    debug: Option<&mut DBGMCU>, // Enable debugging during sleep, FCLK and HCLK remain on during sleep
    scb: &mut SCB,
    pwr: &mut PWR,
    nvic: &mut NVIC, // Required for RTC alarm and WKUP pin
    exti: &mut EXTI, // Required for RTC alarm and WKUP pin
    apb1: &mut APB1) // Required for WKUP pin
{
    // Set SLEEPDEEP in System Control Register
    match sleep_mode {
        SleepMode::Sleep => scb.clear_sleepdeep(),
        _ => scb.set_sleepdeep(),
    }

    // Set the PDDS and LPDS bit in the Power Control Register
    match sleep_mode {
        SleepMode::Sleep =>
            pwr.cr.modify(|_, w| w.pdds().clear_bit()),
        SleepMode::StopRegulatorOn =>
            // Voltage regulator ON
            pwr.cr.modify(|_, w|
                w.pdds().clear_bit()
                 .lpds().clear_bit()
            ),
        SleepMode::StopRegulatorLP =>
            // Voltage regulator in low power mode
            pwr.cr.modify(|_, w|
                w.pdds().clear_bit()
                 .lpds().set_bit()
            ),
        SleepMode::Standby =>
            // Voltage regulator OFF
            pwr.cr.modify(|_, w| w.pdds().set_bit()),
    };

    let standby_flag = pwr.csr.read().sbf().bit();
    if standby_flag {
        // Clear standby flag
        pwr.cr.modify(|_, w| w.csbf().set_bit());
    }

    let wakeup_flag = pwr.csr.read().wuf().bit();
    if wakeup_flag {
        // A Wakeup event was received from the WKUP pin or from the RTC alarm,
        // clear the Wakeup flag
        pwr.cr.modify(|_, w| w.cwuf().set_bit());
    }

    let is_sleep_or_stop_mode = match sleep_mode {
        SleepMode::Sleep => true,
        SleepMode::StopRegulatorOn => true,
        SleepMode::StopRegulatorLP => true,
        _ => false,
    };

    // Wake up the MCU using the RTC alarm.
    // Note: the user needs to call `rtc.set_alarm()`!
    if enable_wakeup_alarm {
        if is_sleep_or_stop_mode {
            // Enable RTC interrupt in NVIC
            nvic.enable(RTC);
            NVIC::unpend(RTC);
            nvic.enable(RTCALARM);
            NVIC::unpend(RTCALARM);

            // 1. Enable line 17 (RTC alarm) in IMR or EMR
            match sleep_mode_entry {
                SleepModeEntry::WFI =>
                    // Interrupt Mask Register
                    exti.imr.modify(|_, w| w.mr17().set_bit()),
                SleepModeEntry::WFE =>
                    // Event Mask Register
                    exti.emr.modify(|_, w| w.mr17().set_bit()),
            };

            // 2. Enable rising edge trigger on line 17
            exti.rtsr.modify(|_, w| w.tr17().set_bit());

            // 3. Clear pending bit for line 17
            exti.pr.modify(|_, w| w.pr17().set_bit());
        }
    }

    // Wake up the MCU using the WKUP pin (PA0)
    if enable_wakeup_pin {
        // Enable power interface clock in RCC_APB1ENR register
        apb1.set_pwren();

        // Enable WKUP pin (PA0)
        pwr.csr.modify(|_, w| w.ewup().set_bit());

        if is_sleep_or_stop_mode {
            // 0. Enable EXTI0
            nvic.enable(EXTI0);
            NVIC::unpend(EXTI0);

            // 1. Enable line 0 (PA0) in the IMR or EMR
            match sleep_mode_entry {
                SleepModeEntry::WFI =>
                    // Interrupt Mask Register
                    exti.imr.modify(|_, w| w.mr0().set_bit()),
                SleepModeEntry::WFE =>
                    // Event Mask Register
                    exti.emr.modify(|_, w| w.mr0().set_bit()),
            }

            // 2. Enable rising edge trigger on line 0
            exti.rtsr.modify(|_, w| w.tr0().set_bit());

            // 3. Clear pending bit for line 0
            exti.pr.modify(|_, w| w.pr0().set_bit());
        }
    }

    // Deep Sleep Debug Mode
    if let Some(dbgmcu) = debug {
        dbgmcu.cr.modify(|_, w|
            w.dbg_sleep().set_bit()
             .dbg_stop().set_bit()
             .dbg_standby().set_bit());
    }

    // Lastly, perform a Wait for interrupt or Wait for event instruction, this
    // will immediately put the MCU to sleep.
    match sleep_mode_entry {
        SleepModeEntry::WFI => asm::wfi(),
        SleepModeEntry::WFE => asm::wfe(),
    }
}
