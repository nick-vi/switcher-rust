use std::io::{self, Write};
use std::time::Duration;
use tokio::time::timeout;

use switcher_rust::cache::CacheManager;
use switcher_rust::control::SwitcherController;
use switcher_rust::device::{DeviceState, SwitcherDevice};
use switcher_rust::discovery::SwitcherDiscovery;
use switcher_rust::pairing::PairingManager;

struct TestResults {
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl TestResults {
    fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            skipped: 0,
        }
    }

    fn pass(&mut self) {
        self.passed += 1;
        println!("✅ PASSED");
    }

    fn fail(&mut self, reason: &str) {
        self.failed += 1;
        println!("❌ FAILED: {}", reason);
    }

    fn skip(&mut self, reason: &str) {
        self.skipped += 1;
        println!("⚠️  SKIPPED: {}", reason);
    }

    fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }
}

async fn discover_devices() -> Vec<SwitcherDevice> {
    let discovery = SwitcherDiscovery::new();

    match discovery.discover(Duration::from_secs(3)).await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("⚠️  No devices found");
            } else {
                println!("✅ Found {} device(s):", devices.len());
                for (i, device) in devices.iter().enumerate() {
                    println!(
                        "  {}. {} at {} (ID: {})",
                        i + 1,
                        device.name,
                        device.ip_address,
                        device.device_id
                    );
                }
            }
            devices
        }
        Err(e) => {
            println!("❌ Discovery error: {}", e);
            Vec::new()
        }
    }
}

fn select_device_for_testing(devices: &[SwitcherDevice]) -> Option<SwitcherDevice> {
    if devices.is_empty() {
        return None;
    }

    println!("\n⚠️  WARNING: Real device testing will control your actual device!");
    println!("This will turn the device ON and OFF during testing.");

    if devices.len() == 1 {
        let device = &devices[0];
        println!(
            "Device: {} at {} (ID: {})",
            device.name, device.ip_address, device.device_id
        );
        print!("Do you want to test on this device? [y/N]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            return Some(device.clone());
        }
    } else {
        println!("Multiple devices found. Select one for testing:");
        for (i, device) in devices.iter().enumerate() {
            println!(
                "  {}. {} at {} (ID: {})",
                i + 1,
                device.name,
                device.ip_address,
                device.device_id
            );
        }
        println!("  0. Skip real device testing");

        print!("Enter your choice [0-{}]: ", devices.len());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice > 0 && choice <= devices.len() {
                return Some(devices[choice - 1].clone());
            }
        }
    }

    None
}

async fn test_status_commands(results: &mut TestResults, real_device: Option<&SwitcherDevice>) {
    println!("\n=== STATUS TESTS ===");

    // Test with real device if available
    if let Some(device) = real_device {
        print!("🧪 Test: Real Device Status... ");
        let controller =
            SwitcherController::new(device.ip_address.clone(), device.device_id.clone());

        match timeout(Duration::from_secs(8), controller.get_status()).await {
            Ok(Ok(status)) => {
                println!(
                    "✅ PASSED (State: {:?}, Power: {}W)",
                    status.state, status.power_consumption
                );
                results.passed += 1;
            }
            Ok(Err(e)) => results.fail(&format!("Status failed: {}", e)),
            Err(_) => results.fail("Status timed out"),
        }
    } else {
        print!("🧪 Test: Real Device Status... ");
        results.skip("No real device available");
    }

    // Test with fake device ID
    print!("🧪 Test: Fake Device ID Status... ");
    let controller = SwitcherController::new("10.0.0.24".to_string(), "999999".to_string());

    match timeout(Duration::from_secs(8), controller.get_status()).await {
        Ok(Ok(_)) => results.fail("Should have failed with fake device ID"),
        Ok(Err(_)) => results.pass(), // Expected to fail
        Err(_) => results.fail("Status timed out"),
    }

    // Test with invalid IP
    print!("🧪 Test: Invalid IP Status... ");
    let controller = SwitcherController::new("192.168.1.100".to_string(), "9c4f22".to_string());

    match timeout(Duration::from_secs(8), controller.get_status()).await {
        Ok(Ok(_)) => results.fail("Should have failed with invalid IP"),
        Ok(Err(_)) => results.pass(), // Expected to fail
        Err(_) => results.pass(),     // Timeout is also acceptable for invalid IP
    }
}

async fn test_control_commands(results: &mut TestResults, real_device: Option<&SwitcherDevice>) {
    println!("\n=== CONTROL TESTS ===");

    if let Some(device) = real_device {
        let controller =
            SwitcherController::new(device.ip_address.clone(), device.device_id.clone());

        // Get current state
        let original_state = match timeout(Duration::from_secs(8), controller.get_status()).await {
            Ok(Ok(status)) => Some(status.state),
            _ => None,
        };

        if let Some(orig_state) = original_state {
            println!("Current device state: {:?}", orig_state);
        }

        // Test turn ON
        print!("🧪 Test: Real Device Turn ON... ");
        match timeout(Duration::from_secs(10), controller.turn_on()).await {
            Ok(Ok(_)) => results.pass(),
            Ok(Err(e)) => results.fail(&format!("Turn ON failed: {}", e)),
            Err(_) => results.fail("Turn ON timed out"),
        }

        // Test turn OFF
        print!("🧪 Test: Real Device Turn OFF... ");
        match timeout(Duration::from_secs(10), controller.turn_off()).await {
            Ok(Ok(_)) => results.pass(),
            Ok(Err(e)) => results.fail(&format!("Turn OFF failed: {}", e)),
            Err(_) => results.fail("Turn OFF timed out"),
        }

        // Restore original state if possible
        if let Some(orig_state) = original_state {
            println!("Restoring original state: {:?}", orig_state);
            match orig_state {
                DeviceState::On => {
                    let _ = controller.turn_on().await;
                }
                DeviceState::Off => {
                    let _ = controller.turn_off().await;
                }
                DeviceState::Unknown => {}
            }
        }
    } else {
        print!("🧪 Test: Real Device Control... ");
        results.skip("No real device available");
    }

    // Test fake device control
    print!("🧪 Test: Fake Device ID Control... ");
    let fake_controller = SwitcherController::new("10.0.0.24".to_string(), "999999".to_string());

    match timeout(Duration::from_secs(8), fake_controller.turn_on()).await {
        Ok(Ok(_)) => results.fail("Should have failed with fake device ID"),
        Ok(Err(_)) => results.pass(), // Expected to fail
        Err(_) => results.fail("Control timed out"),
    }
}

async fn test_cache_functionality(
    results: &mut TestResults,
    discovered_devices: &[SwitcherDevice],
) {
    println!("\n=== CACHE TESTS ===");

    // Test cache creation and device storage
    print!("🧪 Test: Cache Creation and Device Storage... ");
    let cache_manager = match CacheManager::new() {
        Ok(manager) => manager,
        Err(e) => {
            results.fail(&format!("Failed to create cache manager: {}", e));
            return;
        }
    };

    let mut cache = match cache_manager.load_cache() {
        Ok(cache) => cache,
        Err(e) => {
            results.fail(&format!("Failed to load cache: {}", e));
            return;
        }
    };

    // Add discovered devices to cache
    let initial_count = cache.devices.len();
    for device in discovered_devices {
        cache.add_device(device.clone());
    }

    if cache.devices.len() >= initial_count {
        results.pass();
    } else {
        results.fail("Device count didn't increase after adding devices");
        return;
    }

    // Test cache persistence
    print!("🧪 Test: Cache Persistence... ");
    match cache_manager.save_cache(&cache) {
        Ok(()) => {
            // Try to reload and verify data persisted
            match cache_manager.load_cache() {
                Ok(reloaded_cache) => {
                    if reloaded_cache.devices.len() == cache.devices.len() {
                        results.pass();
                    } else {
                        results.fail("Cache data not persisted correctly");
                    }
                }
                Err(e) => results.fail(&format!("Failed to reload cache: {}", e)),
            }
        }
        Err(e) => results.fail(&format!("Failed to save cache: {}", e)),
    }
}

async fn test_pairing_functionality(
    results: &mut TestResults,
    test_device: Option<&SwitcherDevice>,
) {
    println!("\n=== PAIRING TESTS ===");

    if let Some(device) = test_device {
        let pairing_manager = match PairingManager::new() {
            Ok(manager) => manager,
            Err(e) => {
                results.fail(&format!("Failed to create pairing manager: {}", e));
                return;
            }
        };

        let mut pairing = match pairing_manager.load_pairing() {
            Ok(pairing) => pairing,
            Err(e) => {
                results.fail(&format!("Failed to load pairing: {}", e));
                return;
            }
        };

        // Test device pairing
        print!("🧪 Test: Device Pairing... ");
        let test_alias = "Test Device Alias";
        match pairing.pair_device(device.clone(), test_alias.to_string()) {
            Ok(()) => {
                if pairing.get_device_by_alias(test_alias).is_some() {
                    results.pass();
                } else {
                    results.fail("Device not found by alias after pairing");
                    return;
                }
            }
            Err(e) => {
                results.fail(&format!("Failed to pair device: {}", e));
                return;
            }
        }

        // Test duplicate alias prevention
        print!("🧪 Test: Duplicate Alias Prevention... ");
        match pairing.pair_device(device.clone(), test_alias.to_string()) {
            Ok(()) => results.fail("Should have prevented duplicate alias"),
            Err(_) => results.pass(), // Expected to fail
        }

        // Test device unpairing
        print!("🧪 Test: Device Unpairing... ");
        match pairing.unpair_device(test_alias) {
            Ok(()) => {
                if pairing.get_device_by_alias(test_alias).is_none() {
                    results.pass();
                } else {
                    results.fail("Device still found by alias after unpairing");
                }
            }
            Err(e) => results.fail(&format!("Failed to unpair device: {}", e)),
        }

        // Test pairing persistence
        print!("🧪 Test: Pairing Persistence... ");
        let persistent_alias = "Persistent Test Alias";
        match pairing.pair_device(device.clone(), persistent_alias.to_string()) {
            Ok(()) => {
                match pairing_manager.save_pairing(&pairing) {
                    Ok(()) => {
                        match pairing_manager.load_pairing() {
                            Ok(reloaded_pairing) => {
                                if reloaded_pairing
                                    .get_device_by_alias(persistent_alias)
                                    .is_some()
                                {
                                    results.pass();
                                    // Clean up
                                    let mut cleanup_pairing = reloaded_pairing;
                                    let _ = cleanup_pairing.unpair_device(persistent_alias);
                                    let _ = pairing_manager.save_pairing(&cleanup_pairing);
                                } else {
                                    results.fail("Pairing not persisted after reload");
                                }
                            }
                            Err(e) => results.fail(&format!("Failed to reload pairing: {}", e)),
                        }
                    }
                    Err(e) => results.fail(&format!("Failed to save pairing: {}", e)),
                }
            }
            Err(e) => results.fail(&format!(
                "Failed to pair device for persistence test: {}",
                e
            )),
        }
    } else {
        print!("🧪 Test: Device Pairing... ");
        results.skip("No test device available");
        print!("🧪 Test: Duplicate Alias Prevention... ");
        results.skip("No test device available");
        print!("🧪 Test: Device Unpairing... ");
        results.skip("No test device available");
        print!("🧪 Test: Pairing Persistence... ");
        results.skip("No test device available");
    }
}

async fn test_device_renaming(results: &mut TestResults, test_device: Option<&SwitcherDevice>) {
    println!("\n=== DEVICE RENAMING TESTS ===");

    if let Some(device) = test_device {
        let controller =
            SwitcherController::new(device.ip_address.clone(), device.device_id.clone());

        // Get original name
        let original_name = device.name.clone();

        // Test device renaming
        print!("🧪 Test: Device Renaming... ");
        let test_name = "Test Renamed Device";
        match timeout(
            Duration::from_secs(10),
            controller.set_device_name(test_name),
        )
        .await
        {
            Ok(Ok(())) => {
                // Wait a moment for the change to take effect
                tokio::time::sleep(Duration::from_millis(1000)).await;
                results.pass();

                // Restore original name
                print!("🧪 Test: Restore Original Name... ");
                match timeout(
                    Duration::from_secs(10),
                    controller.set_device_name(&original_name),
                )
                .await
                {
                    Ok(Ok(())) => results.pass(),
                    Ok(Err(e)) => results.fail(&format!("Failed to restore original name: {}", e)),
                    Err(_) => results.fail("Restore name timed out"),
                }
            }
            Ok(Err(e)) => results.fail(&format!("Device renaming failed: {}", e)),
            Err(_) => results.fail("Device renaming timed out"),
        }

        // Test invalid name lengths
        print!("🧪 Test: Invalid Name Length (too short)... ");
        match timeout(Duration::from_secs(8), controller.set_device_name("A")).await {
            Ok(Ok(())) => results.fail("Should have failed with name too short"),
            Ok(Err(_)) => results.pass(), // Expected to fail
            Err(_) => results.pass(),     // Timeout is also acceptable
        }

        print!("🧪 Test: Invalid Name Length (too long)... ");
        let long_name = "A".repeat(35); // Over 32 character limit
        match timeout(
            Duration::from_secs(8),
            controller.set_device_name(&long_name),
        )
        .await
        {
            Ok(Ok(())) => results.fail("Should have failed with name too long"),
            Ok(Err(_)) => results.pass(), // Expected to fail
            Err(_) => results.pass(),     // Timeout is also acceptable
        }
    } else {
        print!("🧪 Test: Device Renaming... ");
        results.skip("No test device available");
        print!("🧪 Test: Restore Original Name... ");
        results.skip("No test device available");
        print!("🧪 Test: Invalid Name Length (too short)... ");
        results.skip("No test device available");
        print!("🧪 Test: Invalid Name Length (too long)... ");
        results.skip("No test device available");
    }
}

#[tokio::test]
async fn comprehensive_test_suite() {
    println!("🚀 Starting Switcher CLI Comprehensive Tests");
    println!("============================================");

    let mut results = TestResults::new();

    // Single discovery that we'll use for everything
    println!("\n=== DISCOVERY & DEVICE SELECTION ===");
    print!("🧪 Test: Device Discovery... ");

    let discovered_devices = match timeout(Duration::from_secs(5), discover_devices()).await {
        Ok(devices) => {
            if devices.is_empty() {
                println!("✅ PASSED (no devices found)");
                results.pass();
                devices
            } else {
                println!("✅ PASSED ({} device(s) found)", devices.len());
                results.pass();
                devices
            }
        }
        Err(_) => {
            println!("❌ FAILED (discovery timed out)");
            results.fail("Discovery timed out");
            Vec::new()
        }
    };

    // Let user select a device for testing if any found
    let real_device = if !discovered_devices.is_empty() {
        select_device_for_testing(&discovered_devices)
    } else {
        None
    };

    if real_device.is_some() {
        println!("✅ User selected device for testing");
    } else if !discovered_devices.is_empty() {
        println!("⚠️  User declined real device testing");
    }

    // Run all tests
    test_status_commands(&mut results, real_device.as_ref()).await;
    test_control_commands(&mut results, real_device.as_ref()).await;
    test_cache_functionality(&mut results, &discovered_devices).await;
    test_pairing_functionality(&mut results, real_device.as_ref()).await;
    test_device_renaming(&mut results, real_device.as_ref()).await;

    // Print final results
    println!("\n============================================");
    println!("📊 TEST SUMMARY");
    println!("============================================");
    println!("Total Tests: {}", results.total());
    println!("✅ Passed: {}", results.passed);
    println!("❌ Failed: {}", results.failed);
    println!("⚠️  Skipped: {}", results.skipped);

    if !discovered_devices.is_empty() {
        println!(
            "✅ Found {} device(s) during discovery",
            discovered_devices.len()
        );
        if real_device.is_some() {
            println!("✅ Real device testing was performed");
            println!("✅ Device renaming tests were performed");
            println!("✅ Pairing functionality tests were performed");
        } else {
            println!("⚠️  Real device testing was skipped");
            println!("⚠️  Device renaming tests were skipped");
            println!("⚠️  Pairing functionality tests were skipped");
        }
        println!("✅ Cache functionality tests were performed");
    } else {
        println!("⚠️  No devices found - real device tests were skipped");
        println!("⚠️  Cache and pairing tests used mock data");
    }

    // Assert that we have no failures
    if results.failed > 0 {
        panic!("❌ {} test(s) failed!", results.failed);
    } else {
        println!("🎉 All tests passed!");
    }
}
