pub fn write_varint(value: i32, buf: &mut [u8], start: usize) -> usize {
	if value == 0 {
		buf[start] = 0;
		return 1_usize;
	}

	let mut value_copy: i32 = value;

	let mut i = 0_usize;

    while value_copy != 0 {
        let mut temp: u8 = (value_copy & 0b01111111) as u8;
        // Note: >>> means that the sign bit is shifted with the rest of the number rather than being left alone
        value_copy >>= 7;
        if value_copy != 0 {
            temp |= 0b10000000;
        }
		buf[start + i] = temp;
		i+=1;
	}
	
	i
}