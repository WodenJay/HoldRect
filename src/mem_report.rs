// src/mem_report.rs
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Returns (working_set_size_kb, pagefile_usage_kb) for the current process.
pub fn get_process_memory_kb() -> Option<(u64, u64)> {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .is_err()
        {
            return None;
        }
        Some((
            counters.WorkingSetSize as u64 / 1024,
            counters.PagefileUsage as u64 / 1024,
        ))
    }
}

/// Print memory report to stdout. For `--mem-report` CLI flag.
pub fn print_mem_report() {
    match get_process_memory_kb() {
        Some((ws, pf)) => {
            println!("HoldRect Memory Report:");
            println!("  Working Set:  {ws} KB ({:.1} MB)", ws as f64 / 1024.0);
            println!("  Pagefile:     {pf} KB ({:.1} MB)", pf as f64 / 1024.0);
        }
        None => {
            eprintln!("Error: GetProcessMemoryInfo failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_process_memory_returns_nonzero() {
        let Some((ws, pf)) = get_process_memory_kb() else {
            panic!("GetProcessMemoryInfo failed");
        };
        assert!(ws > 0, "working set must be > 0, got {ws}");
        assert!(pf > 0, "pagefile usage must be > 0, got {pf}");
    }

    #[test]
    fn get_process_memory_reasonable_range() {
        let Some((ws, _pf)) = get_process_memory_kb() else {
            return;
        };
        // Current HoldRect uses <50MB. Assert <500MB as sanity check.
        assert!(ws < 500_000, "working set {ws} KB seems unreasonably high");
    }
}
