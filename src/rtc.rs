use x86_64::instructions::port::Port;

unsafe fn leer_cmos(reg: u8) -> u8 {
    let mut idx: Port<u8> = Port::new(0x70);
    let mut datos: Port<u8> = Port::new(0x71);
    idx.write(reg);
    datos.read()
}

fn bcd_a_binario(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

/// Devuelve (hora, min, seg, dia, mes, anio_2dig) del RTC CMOS.
pub fn leer_fecha_hora() -> (u8, u8, u8, u8, u8, u8) {
    unsafe {
        while leer_cmos(0x0A) & 0x80 != 0 {}
        let reg_b = leer_cmos(0x0B);
        let bcd = (reg_b & 0x04) == 0;
        let es_12h = (reg_b & 0x02) == 0;
        let mut seg = leer_cmos(0x00);
        let mut min = leer_cmos(0x02);
        let mut hora_raw = leer_cmos(0x04);
        let mut dia = leer_cmos(0x07);
        let mut mes = leer_cmos(0x08);
        let mut anio = leer_cmos(0x09);
        let pm = es_12h && (hora_raw & 0x80) != 0;
        hora_raw &= 0x7F;
        if bcd {
            seg = bcd_a_binario(seg);
            min = bcd_a_binario(min);
            hora_raw = bcd_a_binario(hora_raw);
            dia = bcd_a_binario(dia);
            mes = bcd_a_binario(mes);
            anio = bcd_a_binario(anio);
        }
        let mut hora = hora_raw;
        if es_12h {
            if pm && hora < 12 { hora += 12; }
            if !pm && hora == 12 { hora = 0; }
        }
        (hora, min, seg, dia, mes, anio)
    }
}

pub fn dia_semana(dia: u8, mes: u8, anio_2dig: u8) -> &'static str {
    if mes < 1 || mes > 12 { return "?"; }
    let anio = anio_2dig as i32 + 2000;
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if mes < 3 { anio - 1 } else { anio };
    let idx = ((y + y/4 - y/100 + y/400 + t[(mes as usize) - 1] + dia as i32) % 7) as usize;
    ["Domingo", "Lunes", "Martes", "Miercoles", "Jueves", "Viernes", "Sabado"][idx]
}
