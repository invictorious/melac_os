use x86_64::instructions::port::Port;

// Reinicia la CPU enviando el comando de reset (0xFE) al controlador PS/2 (puerto 0x64)
pub fn reiniciar_sistema() -> ! {
    unsafe {
        let mut puerto_reset: Port<u8> = Port::new(0x64);
        puerto_reset.write(0xFE);
    }
    loop {
        x86_64::instructions::hlt();
    }
}

// Apaga la máquina virtual QEMU enviando la señal ACPI al puerto 0x604
pub fn apagar_sistema() -> ! {
    unsafe {
        let mut puerto_acpi: Port<u16> = Port::new(0x604);
        puerto_acpi.write(0x2000);
    }
    loop {
        x86_64::instructions::hlt();
    }
}

// Emite un pitido real usando el chip temporizador PIT (canal 2) y el PC Speaker (puerto 0x61)
pub fn emitir_beep() {
    unsafe {
        let frecuencia: u32 = 1193180 / 880; // Tono La (880 Hz)
        let mut puerto_cmd: Port<u8> = Port::new(0x43);
        let mut puerto_datos: Port<u8> = Port::new(0x42);
        let mut puerto_altavoz: Port<u8> = Port::new(0x61);

        puerto_cmd.write(0xB6);
        puerto_datos.write((frecuencia & 0xFF) as u8);
        puerto_datos.write((frecuencia >> 8) as u8);

        let estado_actual = puerto_altavoz.read();
        puerto_altavoz.write(estado_actual | 3); // Enciende el altavoz

        // Pequeña pausa activa para que suene el pitido
        for _ in 0..5_000_000 {
            core::hint::spin_loop();
        }

        let estado_final = puerto_altavoz.read();
        puerto_altavoz.write(estado_final & 0xFC); // Apaga el altavoz
    }
}
