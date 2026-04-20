/*
 * Эта лицензия Source Code Form подпадает под условия Mozilla Public
 * License, v. 2.0.
 * Если копия MPL не распространялась вместе с этим
 * файлом, вы можете получить ее на https://mozilla.org/MPL/2.0/.
 */
//! `AudioUnit.h` (Audio Unit Services)

use std::time::Instant;

use crate::audio::openal as al;
use crate::audio::openal::al_types::{ALuint, ALvoid};
use crate::audio::openal::{AL_BUFFERS_PROCESSED, AL_PLAYING, AL_SOURCE_STATE};

const AL_POSITION: i32 = 0x1004;
const AL_REFERENCE_DISTANCE: i32 = 0x1020;
const AL_ROLLOFF_FACTOR: i32 = 0x1021;
const AL_MAX_DISTANCE: i32 = 0x1023;

use crate::abi::CallFromHost;
use crate::dyld::FunctionExports;
use crate::environment::Environment;
use crate::export_c_func;
use crate::frameworks::audio_toolbox::audio_components;
use crate::frameworks::audio_toolbox::audio_queue::{
    is_supported_audio_format, log_if_broken_audio_format,
};
use crate::frameworks::carbon_core::{paramErr, OSStatus};
use crate::frameworks::core_audio_types::{AudioStreamBasicDescription, fourcc};
use crate::frameworks::core_foundation::cf_run_loop::CFRunLoopGetMain;
use crate::frameworks::foundation::ns_run_loop;
use crate::mem::{guest_size_of, ConstVoidPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::objc::nil;

use super::audio_components::{AURenderCallbackStruct, AudioComponentInstance};
use super::audio_queue::decode_buffer;
use super::audio_session;

pub type AudioUnit = AudioComponentInstance;

type AudioUnitPropertyID = u32;
type AudioUnitScope = u32;
type AudioUnitElement = u32;
type AudioUnitParameterID = u32;
type AudioUnitParameterValue = f32;

// =========================================================================
// MARK: - Структуры
// =========================================================================

#[repr(C, packed)]
pub struct AudioBufferList<const COUNT: usize> {
    pub number_buffers: u32,
    pub buffers: [AudioBuffer; COUNT],
}
unsafe impl SafeRead for AudioBufferList<1> {}
unsafe impl SafeRead for AudioBufferList<2> {}

#[repr(C, packed)]
pub struct AudioBuffer {
    pub number_channels: u32,
    pub data_byte_size: u32,
    pub data: MutVoidPtr,
}

/// `AudioUnitConnection` — используется для kAudioUnitProperty_MakeConnection.
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct AudioUnitConnection {
    source_audio_unit:  AudioUnit,
    source_output_number: u32,
    dest_input_number:    u32,
}
unsafe impl SafeRead for AudioUnitConnection {}

// =========================================================================
// MARK: - Константы Scope / element
// =========================================================================

const kAudioUnitScope_Global: AudioUnitScope = 0;
const kAudioUnitScope_Input:  AudioUnitScope = 1;
const kAudioUnitScope_Output: AudioUnitScope = 2;
const kAudioUnitScope_Group:  AudioUnitScope = 3;
const kAudioUnitScope_Part:   AudioUnitScope = 4;
const kAudioUnitScope_Note:   AudioUnitScope = 5;

// =========================================================================
// MARK: - Константы Property ID
// =========================================================================

const kAudioUnitProperty_ClassInfo:              AudioUnitPropertyID = 0;
const kAudioUnitProperty_MakeConnection:         AudioUnitPropertyID = 1;
const kAudioUnitProperty_SampleRate:             AudioUnitPropertyID = 2;
const kAudioUnitProperty_ParameterList:          AudioUnitPropertyID = 3;
const kAudioUnitProperty_ParameterInfo:          AudioUnitPropertyID = 4;
const kAudioUnitProperty_CPULoad:                AudioUnitPropertyID = 6;
const kAudioUnitProperty_StreamFormat:           AudioUnitPropertyID = 8;
const kAudioUnitProperty_ElementCount:           AudioUnitPropertyID = 11;
const kAudioUnitProperty_Latency:                AudioUnitPropertyID = 12;
const kAudioUnitProperty_SupportedNumChannels:   AudioUnitPropertyID = 13;
const kAudioUnitProperty_MaximumFramesPerSlice:  AudioUnitPropertyID = 14;
const kAudioUnitProperty_ParameterValueStrings:  AudioUnitPropertyID = 16;
const kAudioUnitProperty_AudioChannelLayout:     AudioUnitPropertyID = 19;
const kAudioUnitProperty_TailTime:               AudioUnitPropertyID = 20;
const kAudioUnitProperty_BypassEffect:           AudioUnitPropertyID = 21;
const kAudioUnitProperty_LastRenderError:        AudioUnitPropertyID = 22;
const kAudioUnitProperty_SetRenderCallback:      AudioUnitPropertyID = 23;
const kAudioUnitProperty_FactoryPresets:         AudioUnitPropertyID = 24;
const kAudioUnitProperty_RenderQuality:          AudioUnitPropertyID = 26;
const kAudioUnitProperty_HostCallbacks:          AudioUnitPropertyID = 27;
const kAudioUnitProperty_InPlaceProcessing:      AudioUnitPropertyID = 29;
const kAudioUnitProperty_ElementName:            AudioUnitPropertyID = 30;
const kAudioUnitProperty_SupportedChannelLayoutTags: AudioUnitPropertyID = 32;
const kAudioUnitProperty_PresentPreset:          AudioUnitPropertyID = 36;
const kAudioUnitProperty_DependentParameters:    AudioUnitPropertyID = 45;
const kAudioUnitProperty_InputSamplesInOutput:   AudioUnitPropertyID = 49;
const kAudioUnitProperty_ShouldAllocateBuffer:   AudioUnitPropertyID = 51;
const kAudioUnitProperty_FrequencyResponse:      AudioUnitPropertyID = 52;
const kAudioUnitProperty_ParameterHistoryInfo:   AudioUnitPropertyID = 53;
const kAudioUnitProperty_NickName:               AudioUnitPropertyID = 54;
const kAudioUnitProperty_OfflineRender:          AudioUnitPropertyID = 37;
const kAudioUnitProperty_ParameterIDName:        AudioUnitPropertyID = 34;
const kAudioOutputUnitProperty_EnableIO:         AudioUnitPropertyID = 2003;
const kAudioOutputUnitProperty_HasIO:            AudioUnitPropertyID = 2006;
const kAudioOutputUnitProperty_StartTime:        AudioUnitPropertyID = 2004;
const kAudioOutputUnitProperty_SetInputCallback: AudioUnitPropertyID = 2005;
const kAudioOutputUnitProperty_IsRunning:        AudioUnitPropertyID = 2001;
const kAudioMixerProperty_Volume:                AudioUnitPropertyID = 7;
const kAudioMixerProperty_Metering:              AudioUnitPropertyID = 1003;
const kAudioUnitProperty_MeteringMode:           AudioUnitPropertyID = 1003;

// 3D Mixer Property IDs
const kAudioUnitProperty_3DMixerDistanceParams:  AudioUnitPropertyID = fourcc(b"3ddp");
const kAudioUnitProperty_MatrixLevels:           AudioUnitPropertyID = fourcc(b"mxmv");
const kAudioUnitProperty_SpatializationAlgorithm:AudioUnitPropertyID = fourcc(b"spat");
const kAudioUnitProperty_3DMixerRenderingFlags:  AudioUnitPropertyID = fourcc(b"3drf");

// 3D Mixer Parameter IDs
const k3DMixerParam_Azimuth: AudioUnitParameterID = 0;
const k3DMixerParam_Elevation: AudioUnitParameterID = 1;
const k3DMixerParam_Distance: AudioUnitParameterID = 2;

// =========================================================================
// MARK: - Инициализация / Деинициализация AudioUnit
// =========================================================================

fn AudioUnitInitialize(env: &mut Environment, in_unit: AudioUnit) -> OSStatus {
    let run_loop = CFRunLoopGetMain(env);
    ns_run_loop::add_audio_unit(env, run_loop, in_unit);
    0
}

fn AudioUnitUninitialize(env: &mut Environment, in_unit: AudioUnit) -> OSStatus {
    let run_loop = CFRunLoopGetMain(env);
    match ns_run_loop::remove_audio_unit(env, run_loop, in_unit) {
        Ok(_) => 0,
        Err(_) => paramErr,
    }
}

// =========================================================================
// MARK: - Установка свойств AudioUnit
// =========================================================================

fn AudioUnitSetProperty(
    env: &mut Environment,
    in_unit: AudioUnit,
    in_id: AudioUnitPropertyID,
    in_scope: AudioUnitScope,
    in_element: AudioUnitElement,
    in_data: ConstVoidPtr,
    _in_data_size: u32,
) -> OSStatus {
    let mut update_al_distance = None;

    // Ограничиваем область видимости заимствования
    {
        let Some(host_object) = audio_components::State::get(&mut env.framework_state)
            .audio_component_instances
            .get_mut(&in_unit)
        else {
            return paramErr;
        };

        match in_id {
            kAudioUnitProperty_3DMixerDistanceParams => {
                let params = env.mem.read::<audio_components::MixerDistanceParams, false>(in_data.cast());
                let bus = host_object.mixer_buses.entry(in_element).or_default();
                bus.distance_params = params;

                // Сохраняем значения для OpenAL, чтобы применить их после завершения borrow
                if let Some(source) = bus.al_source {
                    update_al_distance = Some((source, params));
                }
            }
            kAudioUnitProperty_MatrixLevels => {
                log_dbg!("Заглушка для kAudioUnitProperty_MatrixLevels на шине {}", in_element);
            }
            kAudioUnitProperty_SpatializationAlgorithm |
            kAudioUnitProperty_3DMixerRenderingFlags => {
                log_dbg!("AudioUnitSetProperty: флаги spatialization/rendering проигнорированы");
            }
            kAudioUnitProperty_SetRenderCallback => {
                let render_callback = env.mem.read::<AURenderCallbackStruct, false>(in_data.cast());
                host_object.render_callback = Some(render_callback);
            }
            kAudioOutputUnitProperty_SetInputCallback => {
                let cb = env.mem.read::<AURenderCallbackStruct, false>(in_data.cast());
                host_object.render_callback = Some(cb);
            }
            kAudioUnitProperty_StreamFormat => {
                let stream_format = env.mem.read::<AudioStreamBasicDescription, false>(in_data.cast());
                log_if_broken_audio_format(&stream_format);
                match in_scope {
                    kAudioUnitScope_Global => host_object.global_stream_format = stream_format,
                    kAudioUnitScope_Output => host_object.output_stream_format = Some(stream_format),
                    kAudioUnitScope_Input  => host_object.input_stream_format  = Some(stream_format),
                    _ => log_dbg!("AudioUnitSetProperty StreamFormat: неподдерживаемая область (scope) {}", in_scope),
                }
            }
            kAudioUnitProperty_SampleRate => {
                let rate: f64 = env.mem.read::<f64, false>(in_data.cast());
                host_object.global_stream_format.sample_rate = rate;
            }
            kAudioUnitProperty_MaximumFramesPerSlice => {
                let frames: u32 = env.mem.read::<u32, false>(in_data.cast());
                host_object.maximum_frames_per_slice = frames;
            }
            kAudioUnitProperty_MakeConnection => {
                let _conn = env.mem.read::<AudioUnitConnection, false>(in_data.cast());
            }
            kAudioOutputUnitProperty_EnableIO => {
                // Из оригинала: Ввод/Вывод включен по умолчанию. Игнорируем или возвращаем успех.
                let enabled = env.mem.read::<u32, false>(in_data.cast());
                log_dbg!("AudioUnitSetProperty EnableIO: {}", enabled);
            }
            _ => {
                log_dbg!("AudioUnitSetProperty: свойство {} проигнорировано", in_id);
            }
        }
    } // Конец заимствования host_object и env.framework_state

    // Теперь безопасно вызываем OpenAL
    if let Some((source, params)) = update_al_distance {
        let context = env.framework_state.audio_toolbox.make_al_context_current(&mut env.openal_manager);
        unsafe {
            context.Sourcef(source, AL_REFERENCE_DISTANCE, params.reference_distance);
            context.Sourcef(source, AL_MAX_DISTANCE, params.maximum_distance);
            context.Sourcef(source, AL_ROLLOFF_FACTOR, params.rolloff_factor);
        }
    }

    0
}

// =========================================================================
// MARK: - Получение свойств AudioUnit
// =========================================================================

fn AudioUnitGetProperty(
    env: &mut Environment,
    in_unit: AudioUnit,
    in_id: AudioUnitPropertyID,
    in_scope: AudioUnitScope,
    _in_element: AudioUnitElement,
    out_data: MutVoidPtr,
    io_data_size: MutPtr<u32>,
) -> OSStatus {
    let Some(host_object) = audio_components::State::get(&mut env.framework_state)
        .audio_component_instances
        .get_mut(&in_unit)
    else {
        return paramErr;
    };

    match in_id {
        kAudioUnitProperty_MaximumFramesPerSlice => {
            let v = host_object.maximum_frames_per_slice;
            env.mem.write(out_data.cast(), v);
            env.mem.write(io_data_size, guest_size_of::<u32>());
        }
        kAudioUnitProperty_StreamFormat => {
            let fmt = match in_scope {
                kAudioUnitScope_Output => host_object.output_stream_format
                    .unwrap_or(host_object.global_stream_format),
                kAudioUnitScope_Input  => host_object.input_stream_format
                    .unwrap_or(host_object.global_stream_format),
                _                      => host_object.global_stream_format,
            };
            env.mem.write(out_data.cast(), fmt);
            env.mem.write(io_data_size, guest_size_of::<AudioStreamBasicDescription>());
        }
        kAudioUnitProperty_SampleRate => {
            let rate = host_object.global_stream_format.sample_rate;
            env.mem.write(out_data.cast(), rate);
            env.mem.write(io_data_size, guest_size_of::<f64>());
        }
        kAudioOutputUnitProperty_IsRunning => {
            let running: u32 = if host_object.started { 1 } else { 0 };
            env.mem.write(out_data.cast(), running);
            env.mem.write(io_data_size, guest_size_of::<u32>());
        }
        _ => return -1,
    }
    0
}

fn AudioUnitGetPropertyInfo(
    env: &mut Environment,
    _in_unit: AudioUnit,
    in_id: AudioUnitPropertyID,
    _in_scope: AudioUnitScope,
    _in_element: AudioUnitElement,
    out_data_size: MutPtr<u32>,
    out_writable: MutPtr<bool>,
) -> OSStatus {
    let (size, writable) = match in_id {
        kAudioUnitProperty_StreamFormat => (guest_size_of::<AudioStreamBasicDescription>(), true),
        kAudioUnitProperty_SampleRate   => (guest_size_of::<f64>(), true),
        kAudioOutputUnitProperty_IsRunning => (guest_size_of::<u32>(), false),
        _ => return -1,
    };

    if !out_data_size.is_null() { env.mem.write(out_data_size, size); }
    if !out_writable.is_null() { env.mem.write(out_writable, writable); }
    0
}

// =========================================================================
// MARK: - Получение/установка параметров (Parameters)
// =========================================================================

fn AudioUnitSetParameter(
    env: &mut Environment,
    in_unit: AudioUnit,
    in_id: AudioUnitParameterID,
    _in_scope: AudioUnitScope,
    in_element: AudioUnitElement,
    in_value: AudioUnitParameterValue,
    _in_offset: u32,
) -> OSStatus {
    let mut update_al_pos = None;

    // Ограничиваем область видимости заимствования
    {
        let Some(host_object) = audio_components::State::get(&mut env.framework_state)
            .audio_component_instances
            .get_mut(&in_unit)
        else {
            return paramErr;
        };

        match in_id {
            k3DMixerParam_Azimuth | k3DMixerParam_Elevation | k3DMixerParam_Distance => {
                let bus = host_object.mixer_buses.entry(in_element).or_default();
                if in_id == k3DMixerParam_Azimuth {
                     let radians = in_value.to_radians();
                     bus.position[0] = radians.sin();
                     bus.position[2] = -radians.cos();
                } else if in_id == k3DMixerParam_Elevation {
                     let radians = in_value.to_radians();
                     bus.position[1] = radians.sin();
                }
                
                // Сохраняем значения для OpenAL
                if let Some(source) = bus.al_source {
                     update_al_pos = Some((source, bus.position));
                }
            }
            _ => {}
        }
    } // Конец заимствования

    // Теперь безопасно вызываем OpenAL
    if let Some((source, pos)) = update_al_pos {
        let context = env.framework_state.audio_toolbox.make_al_context_current(&mut env.openal_manager);
        unsafe {
             context.Source3f(source, AL_POSITION, pos[0], pos[1], pos[2]);
        }
    }

    0
}

fn AudioUnitGetParameter(
    env: &mut Environment,
    _in_unit: AudioUnit,
    _in_id: AudioUnitParameterID,
    _in_scope: AudioUnitScope,
    _in_element: AudioUnitElement,
    out_value: MutPtr<AudioUnitParameterValue>,
) -> OSStatus {
    if !out_value.is_null() {
        env.mem.write(out_value, 1.0);
    }
    0
}

fn AudioUnitScheduleParameters(_e: &mut Environment, _u: AudioUnit, _p: ConstVoidPtr, _n: u32) -> OSStatus { 0 }

fn AudioUnitReset(env: &mut Environment, in_unit: AudioUnit, _s: AudioUnitScope, _e: AudioUnitElement) -> OSStatus {
    if let Some(obj) = audio_components::State::get(&mut env.framework_state).audio_component_instances.get_mut(&in_unit) {
        obj.last_render_time = None;
    }
    0
}

// =========================================================================
// MARK: - Запуск / Остановка AudioOutputUnit
// =========================================================================

fn AudioOutputUnitStart(env: &mut Environment, ci: AudioUnit) -> OSStatus {
    let context = env.framework_state.audio_toolbox.make_al_context_current(&mut env.openal_manager);
    let mut source: ALuint = 0;
    unsafe {
        context.GenSources(1, &mut source);
        context.SourcePlay(source);
    }

    let audio_components_state = audio_components::State::get(&mut env.framework_state);
    let Some(audio_unit_state) = audio_components_state.audio_component_instances.get_mut(&ci) else {
        return paramErr;
    };
    audio_unit_state.al_source = Some(source);
    audio_unit_state.last_render_time = Some(Instant::now());
    audio_unit_state.started = true;
    0
}

fn AudioOutputUnitStop(env: &mut Environment, ci: AudioUnit) -> OSStatus {
    let at_state = &mut env.framework_state.audio_toolbox;
    let context = at_state.al_context.make_al_context_current(&mut env.openal_manager);

    if let Some(audio_unit_state) = at_state.audio_components.audio_component_instances.get_mut(&ci) {
        audio_unit_state.started = false;
        if let Some(al_source) = audio_unit_state.al_source {
            unsafe { context.DeleteSources(1, &al_source); }
        }
        audio_unit_state.al_source = None;
        0
    } else {
        -1
    }
}

// =========================================================================
// MARK: - Рендеринг (Render)
// =========================================================================

fn AudioUnitAddRenderNotify(_e: &mut Environment, _u: AudioUnit, _p: ConstVoidPtr, _r: ConstVoidPtr) -> OSStatus { 0 }
fn AudioUnitRemoveRenderNotify(_e: &mut Environment, _u: AudioUnit, _p: ConstVoidPtr, _r: ConstVoidPtr) -> OSStatus { 0 }

fn AudioUnitRender(env: &mut Environment, in_unit: AudioUnit, _f: MutPtr<u32>, _t: ConstVoidPtr, _b: u32, _n: u32, _d: MutVoidPtr) -> OSStatus {
    render_audio_unit(env, in_unit);
    0
}

fn AudioUnitProcess(env: &mut Environment, in_unit: AudioUnit, _f: MutPtr<u32>, _t: ConstVoidPtr, _n: u32, _d: MutVoidPtr) -> OSStatus {
    render_audio_unit(env, in_unit);
    0
}

fn AudioUnitProcessMultiple(_e: &mut Environment, _u: AudioUnit, _f: MutPtr<u32>, _t: ConstVoidPtr, _n: u32, _in_b: u32, _in_bl: ConstVoidPtr, _out_bl: MutVoidPtr) -> OSStatus { 0 }
fn AudioUnitComplexRender(_e: &mut Environment, _u: AudioUnit, _f: MutPtr<u32>, _t: ConstVoidPtr, _b: u32, _n: u32, _p: MutPtr<u32>, _pd: MutVoidPtr, _d: MutVoidPtr) -> OSStatus { 0 }

pub fn render_audio_unit(env: &mut Environment, audio_unit: AudioUnit) {
    if env.bundle.bundle_identifier().starts_with("com.ea.simcity") { 
        // Применяем хак специфичный для SimCity: пропускаем рендеринг
        return; 
    }

    let (sample_rate, started, is_running, stream_format, has_input_format, al_source, last_render_time, callback) = {
        let at = &mut env.framework_state.audio_toolbox;
        let Some(obj) = at.audio_components.audio_component_instances.get_mut(&audio_unit) else { return; };
        (
            obj.input_stream_format.map(|f| f.sample_rate).unwrap_or(at.audio_session.current_hardware_sample_rate),
            obj.started, obj.is_running_handler,
            obj.input_stream_format.unwrap_or(obj.output_stream_format.unwrap_or(obj.global_stream_format)),
            obj.input_stream_format.is_some(),
            obj.al_source, obj.last_render_time, obj.render_callback,
        )
    };

    if !started || is_running { return; }

    if let Some(obj) = env.framework_state.audio_toolbox.audio_components.audio_component_instances.get_mut(&audio_unit) {
        obj.is_running_handler = true;
    }

    let Some(al_source) = al_source else { return; };
    let Some(last_render_time) = last_render_time else { return; };
    let Some(callback) = callback else { return; };

    let mut al_buffers = Vec::new();
    {
        let context = env.framework_state.audio_toolbox.al_context.make_al_context_current(&mut env.openal_manager);
        unsafe {
            let mut processed = 0;
            context.GetSourcei(al_source, AL_BUFFERS_PROCESSED, &mut processed);
            while processed > 0 {
                let mut b = 0;
                context.SourceUnqueueBuffers(al_source, 1, &mut b);
                al_buffers.push(b);
                context.GetSourcei(al_source, AL_BUFFERS_PROCESSED, &mut processed);
            }
        }
    }

    let now = Instant::now();
    let elapsed = now.duration_since(last_render_time);
    let frames = ((elapsed.as_secs_f64() * sample_rate) as u32).min(2048);
    let buffer_size = frames * stream_format.channels_per_frame * (stream_format.bits_per_channel / 8);

    let action_flags = env.mem.alloc_and_write(0u32);
    
    // Восстанавливаем логику из оригинала: Resident Evil 4 ожидает 2 буфера
    let (audio_buffer_list, buffer1Data, buffer2Data): (MutVoidPtr, MutVoidPtr, Option<MutVoidPtr>) = if has_input_format {
        let bufferData = env.mem.alloc(buffer_size);
        let abl = env.mem.alloc_and_write(AudioBufferList::<1> {
            number_buffers: 1,
            buffers: [AudioBuffer {
                number_channels: stream_format.channels_per_frame,
                data_byte_size:  buffer_size,
                data:            bufferData,
            }],
        });
        (abl.cast(), bufferData, None)
    } else {
        let buffer1Data = env.mem.alloc(buffer_size);
        let buffer2Data = env.mem.alloc(buffer_size);
        let abl = env.mem.alloc_and_write(AudioBufferList::<2> {
            number_buffers: 2,
            buffers: [
                AudioBuffer {
                    number_channels: stream_format.channels_per_frame,
                    data_byte_size:  buffer_size,
                    data:            buffer1Data,
                },
                AudioBuffer {
                    number_channels: stream_format.channels_per_frame,
                    data_byte_size:  buffer_size,
                    data:            buffer2Data,
                },
            ],
        });
        (abl.cast(), buffer1Data, Some(buffer2Data))
    };

    let input_proc = callback.input_proc;
    let input_proc_ref_con = callback.input_proc_ref_con;

    let _: OSStatus = input_proc.call_from_host(env, (
        input_proc_ref_con, action_flags, nil.cast_void().cast_const(), 0u32, frames, audio_buffer_list,
    ));

    let (al_fmt, _, processed) = decode_buffer(&env.mem, &stream_format, buffer1Data.cast(), buffer_size);
    {
        let context = env.framework_state.audio_toolbox.al_context.make_al_context_current(&mut env.openal_manager);
        unsafe {
            let b = al_buffers.pop().unwrap_or_else(|| { let mut x = 0; context.GenBuffers(1, &mut x); x });
            context.BufferData(b, al_fmt, processed.as_ptr() as *const ALvoid, processed.len() as i32, sample_rate as i32);
            context.SourceQueueBuffers(al_source, 1, &b);
            let mut state = 0;
            context.GetSourcei(al_source, AL_SOURCE_STATE, &mut state);
            if state != AL_PLAYING { context.SourcePlay(al_source); }
            if !al_buffers.is_empty() { context.DeleteBuffers(al_buffers.len() as i32, al_buffers.as_ptr()); }
        }
    }

    env.mem.free(action_flags.cast_void());
    env.mem.free(buffer1Data.cast_void());
    if let Some(b2) = buffer2Data {
        env.mem.free(b2.cast_void());
    }
    env.mem.free(audio_buffer_list.cast_void());

    if let Some(obj) = env.framework_state.audio_toolbox.audio_components.audio_component_instances.get_mut(&audio_unit) {
        obj.last_render_time = Some(now);
        obj.is_running_handler = false;
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(AudioUnitInitialize(_)),
    export_c_func!(AudioUnitUninitialize(_)),
    export_c_func!(AudioUnitSetProperty(_, _, _, _, _, _)),
    export_c_func!(AudioUnitGetProperty(_, _, _, _, _, _)),
    export_c_func!(AudioUnitGetPropertyInfo(_, _, _, _, _, _)),
    export_c_func!(AudioUnitSetParameter(_, _, _, _, _, _)),
    export_c_func!(AudioUnitGetParameter(_, _, _, _, _)),
    export_c_func!(AudioUnitScheduleParameters(_, _, _)),
    export_c_func!(AudioUnitReset(_, _, _)),
    export_c_func!(AudioOutputUnitStart(_)),
    export_c_func!(AudioOutputUnitStop(_)),
    export_c_func!(AudioUnitAddRenderNotify(_, _, _)),
    export_c_func!(AudioUnitRemoveRenderNotify(_, _, _)),
    export_c_func!(AudioUnitRender(_, _, _, _, _, _)),
    export_c_func!(AudioUnitProcess(_, _, _, _, _)),
    export_c_func!(AudioUnitProcessMultiple(_, _, _, _, _, _, _)),
];
