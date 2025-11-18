// src/linker.rs

use std::process::Command;
use std::path::Path;

pub struct Linker;

impl Linker {
   
    pub fn link_executable(
        object_file: &str, 
        output_exe: &str,
        runtime_lib_dir: &str,
        enable_lto: bool,
    ) -> Result<(), String> {
        println!("🔗 Phase 5: Linking");
        
   
        let runtime_lib = if cfg!(target_os = "windows") {
            format!("{}/quantica.lib", runtime_lib_dir)
        } else {
            format!("{}/libquantica.a", runtime_lib_dir)
        };
        
        
        if !Path::new(&runtime_lib).exists() {
            return Err(format!(
                "Runtime library not found: {}\n   \
                Please build the runtime library first with: cargo build",
                runtime_lib
            ));
        }
        
       
        if !Path::new(object_file).exists() {
            return Err(format!("Object file not found: {}", object_file));
        }
        
   
        #[cfg(target_os = "windows")]
        {
            Self::try_link_windows(object_file, output_exe, &runtime_lib,enable_lto)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            Self::try_link_unix(object_file, output_exe, &runtime_lib,enable_lto)
        }
    }
    
    #[cfg(target_os = "windows")]
    fn try_link_windows(
        object_file: &str, 
        output_exe: &str, 
        runtime_lib: &str,
        enable_lto: bool,
    ) -> Result<(), String> {

        let lto_args: Vec<&str> = if enable_lto { 
         
            println!("   -> Enabling ThinLTO (LTCG mode)...");
            vec!["/LTCG"] 
        } else { 
            vec![] 
        };

        
        println!("   -> Trying clang linker...");
        let clang_result = Command::new("clang")
            .args(lto_args.iter().cloned())
            .args(&[
                object_file,
                runtime_lib,
                "-o", output_exe,
                "-Wl,/subsystem:console",
            ])
            .output();
        
        if let Ok(output) = clang_result {
            if output.status.success() {
                println!("   -> Linked with clang");
                return Ok(());
            }
        }
        
       
        println!("   -> Trying MSVC link.exe...");
        let link_result = Command::new("link.exe")
            .args(lto_args.iter().cloned())
            .args(&[
                object_file,
                runtime_lib,
                &format!("/OUT:{}", output_exe),
                "/SUBSYSTEM:CONSOLE",
                "/NOLOGO",
       
                "ws2_32.lib",      // Winsock (networking)
                "advapi32.lib",    // Advanced Windows API
                "userenv.lib",     // User environment
                "ntdll.lib",       // NT kernel functions
                "bcrypt.lib",      // Cryptography
                "kernel32.lib",    // Core Windows functions
                "msvcrt.lib",      // C runtime
            ])
            .output();
        
        if let Ok(output) = link_result {
            if output.status.success() {
                println!("   -> Linked with MSVC link.exe");
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() && stderr.len() < 500 {
                println!("   -> link.exe: {}", stderr.trim());
            }
        }
        
       
        println!("   -> Trying ld (MinGW)...");
        let ld_lto_arg = if enable_lto { "-Wl,--lto-thin" } else { "" };
        let ld_result = Command::new("ld")
            .args(&[
                ld_lto_arg,
                object_file,
                runtime_lib,
                "-o", output_exe,
                "-lmsvcrt",
            ])
            .output();
        
        if let Ok(output) = ld_result {
            if output.status.success() {
                println!("   -> Linked with ld");
                return Ok(());
            }
        }
        
        Err(
            "No suitable linker found.\n   \
            Tried: rustc, link.exe (MSVC), ld (MinGW)\n\n   \
            Please try manual linking:\n   \
            1. Open 'x64 Native Tools Command Prompt for VS'\n   \
            2. Run: link output.o target\\debug\\quantica.lib /OUT:test_compile.exe /SUBSYSTEM:CONSOLE kernel32.lib msvcrt.lib".to_string()
        )
    }
    
    #[cfg(not(target_os = "windows"))]
    fn try_link_unix(
        object_file: &str, 
        output_exe: &str, 
        runtime_lib: &str,
        enable_lto: bool,
    ) -> Result<(), String> {
        let lto_arg = if enable_lto { "-flto=thin" } else { "" };
       
        println!("   -> Trying clang linker...");
        let clang_result = Command::new("clang")
            .arg(lto_arg)
            .args(&[
                object_file,
                runtime_lib,
                "-o", output_exe,
                "-lm",
            ])
            .output();
        
        if let Ok(output) = clang_result {
            if output.status.success() {
                println!("   -> Linked with clang");
                return Ok(());
            }
        }
        
    
        println!("   -> Trying gcc linker...");
        let gcc_result = Command::new("gcc")
            .arg(lto_arg)
            .args(&[
                object_file,
                runtime_lib,
                "-o", output_exe,
                "-lm",
            ])
            .output();
        
        if let Ok(output) = gcc_result {
            if output.status.success() {
                println!("   -> Linked with gcc");
                return Ok(());
            }
            return Err(format!(
                "gcc failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        
        Err("No suitable linker found. Please install clang or gcc.".to_string())
    }

}
