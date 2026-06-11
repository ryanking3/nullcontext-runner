#[cfg(any(target_os = "linux", target_os = "windows"))]
use core::ffi::c_void;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use libloading::Library;

pub struct CudaPressureProbeReport {
    pub status: String,
    pub notes: Vec<String>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
const CUDA_PRESSURE_TARGET_FRACTION: usize = 4;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const CUDA_PRESSURE_MAX_BYTES: usize = 512 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const CUDA_PRESSURE_MIN_BYTES: usize = 64 * 1024 * 1024;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const CUDA_PRESSURE_CHUNK_BYTES: usize = 64 * 1024 * 1024;

pub fn run_cuda_memory_pressure_probe() -> CudaPressureProbeReport {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        match run_cuda_memory_pressure_probe_impl() {
            Ok(report) => report,
            Err(error) => CudaPressureProbeReport {
                status: "cuda_memory_pressure_probe_failed".to_string(),
                notes: vec![format!("CUDA memory pressure probe failed: {error}.")],
            },
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        CudaPressureProbeReport {
            status: "cuda_memory_pressure_probe_unsupported_on_platform".to_string(),
            notes: vec![
                "CUDA memory pressure probing is only implemented for Windows/Linux builds right now."
                    .to_string(),
            ],
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuResult = i32;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuDevice = i32;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuContext = *mut c_void;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuDevicePtr = u64;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuInit = unsafe extern "C" fn(flags: u32) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuDeviceGetCount = unsafe extern "C" fn(count: *mut i32) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuDeviceGet = unsafe extern "C" fn(device: *mut CuDevice, ordinal: i32) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuCtxCreateV2 =
    unsafe extern "C" fn(ctx: *mut CuContext, flags: u32, dev: CuDevice) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuCtxDestroyV2 = unsafe extern "C" fn(ctx: CuContext) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuMemGetInfoV2 = unsafe extern "C" fn(free: *mut usize, total: *mut usize) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuMemAllocV2 = unsafe extern "C" fn(ptr: *mut CuDevicePtr, bytesize: usize) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuMemFreeV2 = unsafe extern "C" fn(ptr: CuDevicePtr) -> CuResult;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type CuMemsetD8V2 = unsafe extern "C" fn(dst_device: CuDevicePtr, uc: u8, n: usize) -> CuResult;

#[cfg(any(target_os = "linux", target_os = "windows"))]
const CUDA_SUCCESS: CuResult = 0;

#[cfg(any(target_os = "linux", target_os = "windows"))]
struct CudaDriverApi {
    _library: Library,
    cu_init: CuInit,
    cu_device_get_count: CuDeviceGetCount,
    cu_device_get: CuDeviceGet,
    cu_ctx_create_v2: CuCtxCreateV2,
    cu_ctx_destroy_v2: CuCtxDestroyV2,
    cu_mem_get_info_v2: CuMemGetInfoV2,
    cu_mem_alloc_v2: CuMemAllocV2,
    cu_mem_free_v2: CuMemFreeV2,
    cu_memset_d8_v2: CuMemsetD8V2,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl CudaDriverApi {
    fn load() -> Result<Self, String> {
        let mut errors = Vec::new();

        for candidate in cuda_driver_library_candidates() {
            let library = unsafe { Library::new(candidate) };

            match library {
                Ok(library) => {
                    let cu_init = unsafe { *library.get::<CuInit>(b"cuInit\0") }
                        .map_err(|error| format!("missing cuInit in {candidate}: {error}"))?;
                    let cu_device_get_count =
                        unsafe { *library.get::<CuDeviceGetCount>(b"cuDeviceGetCount\0") }
                            .map_err(|error| {
                                format!("missing cuDeviceGetCount in {candidate}: {error}")
                            })?;
                    let cu_device_get = unsafe { *library.get::<CuDeviceGet>(b"cuDeviceGet\0") }
                        .map_err(|error| format!("missing cuDeviceGet in {candidate}: {error}"))?;
                    let cu_ctx_create_v2 =
                        unsafe { *library.get::<CuCtxCreateV2>(b"cuCtxCreate_v2\0") }.map_err(
                            |error| format!("missing cuCtxCreate_v2 in {candidate}: {error}"),
                        )?;
                    let cu_ctx_destroy_v2 =
                        unsafe { *library.get::<CuCtxDestroyV2>(b"cuCtxDestroy_v2\0") }.map_err(
                            |error| format!("missing cuCtxDestroy_v2 in {candidate}: {error}"),
                        )?;
                    let cu_mem_get_info_v2 =
                        unsafe { *library.get::<CuMemGetInfoV2>(b"cuMemGetInfo_v2\0") }.map_err(
                            |error| format!("missing cuMemGetInfo_v2 in {candidate}: {error}"),
                        )?;
                    let cu_mem_alloc_v2 =
                        unsafe { *library.get::<CuMemAllocV2>(b"cuMemAlloc_v2\0") }.map_err(
                            |error| format!("missing cuMemAlloc_v2 in {candidate}: {error}"),
                        )?;
                    let cu_mem_free_v2 = unsafe { *library.get::<CuMemFreeV2>(b"cuMemFree_v2\0") }
                        .map_err(|error| format!("missing cuMemFree_v2 in {candidate}: {error}"))?;
                    let cu_memset_d8_v2 =
                        unsafe { *library.get::<CuMemsetD8V2>(b"cuMemsetD8_v2\0") }.map_err(
                            |error| format!("missing cuMemsetD8_v2 in {candidate}: {error}"),
                        )?;

                    return Ok(Self {
                        _library: library,
                        cu_init,
                        cu_device_get_count,
                        cu_device_get,
                        cu_ctx_create_v2,
                        cu_ctx_destroy_v2,
                        cu_mem_get_info_v2,
                        cu_mem_alloc_v2,
                        cu_mem_free_v2,
                        cu_memset_d8_v2,
                    });
                }
                Err(error) => errors.push(format!("{candidate}: {error}")),
            }
        }

        Err(format!(
            "CUDA driver library unavailable ({})",
            errors.join("; ")
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn cuda_driver_library_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["nvcuda.dll"]
    }

    #[cfg(target_os = "linux")]
    {
        &["libcuda.so.1", "libcuda.so"]
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn run_cuda_memory_pressure_probe_impl() -> Result<CudaPressureProbeReport, String> {
    let api = CudaDriverApi::load()?;
    let init_result = unsafe { (api.cu_init)(0) };
    ensure_cuda_success("cuInit", init_result)?;

    let mut device_count = 0_i32;
    ensure_cuda_success("cuDeviceGetCount", unsafe {
        (api.cu_device_get_count)(&mut device_count as *mut i32)
    })?;

    if device_count <= 0 {
        return Ok(CudaPressureProbeReport {
            status: "cuda_memory_pressure_probe_no_cuda_devices".to_string(),
            notes: vec!["CUDA driver loaded, but no CUDA devices were reported.".to_string()],
        });
    }

    let mut device = 0_i32;
    ensure_cuda_success("cuDeviceGet", unsafe {
        (api.cu_device_get)(&mut device as *mut i32, 0)
    })?;

    let mut context = std::ptr::null_mut();
    ensure_cuda_success("cuCtxCreate_v2", unsafe {
        (api.cu_ctx_create_v2)(&mut context as *mut CuContext, 0, device)
    })?;

    let result = run_cuda_pressure_with_context(&api);
    let destroy_result = unsafe { (api.cu_ctx_destroy_v2)(context) };
    let destroy_error = ensure_cuda_success("cuCtxDestroy_v2", destroy_result).err();

    match (result, destroy_error) {
        (Ok(mut report), None) => {
            report
                .notes
                .push("CUDA pressure probe destroyed its temporary CUDA context.".to_string());
            Ok(report)
        }
        (Ok(mut report), Some(error)) => {
            report.notes.push(format!(
                "CUDA pressure probe finished, but CUDA context teardown reported an error: {error}."
            ));
            report.status = "cuda_memory_pressure_probe_completed_with_cleanup_warning".to_string();
            Ok(report)
        }
        (Err(error), None) => Err(error),
        (Err(error), Some(destroy_error)) => Err(format!("{error}; cleanup: {destroy_error}")),
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn run_cuda_pressure_with_context(api: &CudaDriverApi) -> Result<CudaPressureProbeReport, String> {
    let mut free_bytes = 0_usize;
    let mut total_bytes = 0_usize;
    ensure_cuda_success("cuMemGetInfo_v2", unsafe {
        (api.cu_mem_get_info_v2)(
            &mut free_bytes as *mut usize,
            &mut total_bytes as *mut usize,
        )
    })?;

    let target_bytes = (free_bytes / CUDA_PRESSURE_TARGET_FRACTION)
        .min(CUDA_PRESSURE_MAX_BYTES)
        .max(0);

    if target_bytes < CUDA_PRESSURE_MIN_BYTES {
        return Ok(CudaPressureProbeReport {
            status: "cuda_memory_pressure_probe_skipped_low_free_memory".to_string(),
            notes: vec![format!(
                "CUDA device reported only {} free bytes before the probe, below the {}-byte minimum target.",
                free_bytes, CUDA_PRESSURE_MIN_BYTES
            )],
        });
    }

    let mut remaining = target_bytes;
    let mut allocations: Vec<(CuDevicePtr, usize)> = Vec::new();
    let mut total_allocated = 0_usize;
    let mut allocation_failures = Vec::new();

    while remaining > 0 {
        let chunk_bytes = remaining.min(CUDA_PRESSURE_CHUNK_BYTES);
        let mut ptr = 0_u64;
        let alloc_result = unsafe { (api.cu_mem_alloc_v2)(&mut ptr as *mut u64, chunk_bytes) };

        if alloc_result != CUDA_SUCCESS {
            allocation_failures.push(format!(
                "cuMemAlloc_v2 failed while requesting {} bytes (result code {}).",
                chunk_bytes, alloc_result
            ));
            break;
        }

        ensure_cuda_success("cuMemsetD8_v2 pattern fill", unsafe {
            (api.cu_memset_d8_v2)(ptr, 0xA5, chunk_bytes)
        })?;
        ensure_cuda_success("cuMemsetD8_v2 zero fill", unsafe {
            (api.cu_memset_d8_v2)(ptr, 0x00, chunk_bytes)
        })?;

        allocations.push((ptr, chunk_bytes));
        total_allocated += chunk_bytes;
        remaining = remaining.saturating_sub(chunk_bytes);
    }

    let mut free_errors = Vec::new();
    while let Some((ptr, _chunk_bytes)) = allocations.pop() {
        let free_result = unsafe { (api.cu_mem_free_v2)(ptr) };
        if free_result != CUDA_SUCCESS {
            free_errors.push(format!(
                "cuMemFree_v2 reported result code {} while releasing a probe allocation.",
                free_result
            ));
        }
    }

    let mut notes = vec![
        format!("CUDA device free bytes before probe: {free_bytes}."),
        format!("CUDA device total bytes before probe: {total_bytes}."),
        format!("CUDA pressure probe target bytes: {target_bytes}."),
        format!("CUDA pressure probe allocated and touched {} bytes.", total_allocated),
        "Each allocated chunk was pattern-filled with 0xA5 and then overwritten with 0x00 before release."
            .to_string(),
    ];

    notes.extend(allocation_failures);
    notes.extend(free_errors);

    let status = if total_allocated == 0 {
        "cuda_memory_pressure_probe_no_allocations_completed".to_string()
    } else if notes
        .iter()
        .any(|note| note.contains("failed") || note.contains("result code"))
    {
        "cuda_memory_pressure_probe_completed_with_warnings".to_string()
    } else {
        "cuda_memory_pressure_probe_completed".to_string()
    };

    Ok(CudaPressureProbeReport { status, notes })
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn ensure_cuda_success(op: &str, result: CuResult) -> Result<(), String> {
    if result == CUDA_SUCCESS {
        Ok(())
    } else {
        Err(format!("{op} returned CUDA driver result code {result}"))
    }
}
