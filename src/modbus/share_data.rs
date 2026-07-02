use std::sync::Arc;
use arc_swap::ArcSwap;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayFormat {
    Binary,
    Octal,
    #[default]
    Decimal,
    Hexadecimal,
}

impl DisplayFormat {
    pub fn format(
        &self,
        value: &DValue,
    ) -> String {
        match value {
            DValue::U16(v) => self.format_u64(*v as u64, 16),
            DValue::I16(v) => match self {
                DisplayFormat::Decimal => v.to_string(),
                _ => self.format_u64(*v as u16 as u64, 16),
            }

            DValue::U32(v) => self.format_u64(*v as u64, 32),
            DValue::I32(v) => match self {
                DisplayFormat::Decimal => v.to_string(),
                _ => self.format_u64(*v as u32 as u64, 32),
            }
            DValue::F32(v) => match self {
                DisplayFormat::Decimal => v.to_string(),
                _ => self.format_u64(v.to_bits() as u32 as u64, 32),
            }

            DValue::U64(v) => self.format_u64(*v, 64),
            DValue::I64(v) => match self {
                DisplayFormat::Decimal => v.to_string(),
                _ => self.format_u64(*v as u64, 64),
            }
            DValue::F64(v) => match self {
                DisplayFormat::Decimal => v.to_string(),
                _ => self.format_u64(v.to_bits(), 64),
            }
        }
    }

    fn format_u64(
        &self,
        value: u64,
        bits: usize,
    ) -> String {
        match self {
            DisplayFormat::Binary => {
                let s = format!("{value:b}");

                let width = bits.max(s.len());

                let padded = format!("{value:0width$b}");

                let groups: Vec<_> = padded
                    .as_bytes()
                    .chunks(4)
                    .map(|c| std::str::from_utf8(c).unwrap())
                    .collect();

                format!("0b_{}", groups.join("_"))
            }

            DisplayFormat::Octal => {
                format!("0o{value:o}")
            }

            DisplayFormat::Decimal => {
                value.to_string()
            }

            DisplayFormat::Hexadecimal => {
                let width = bits / 4;
                format!("0x{value:0width$X}")
            }
        }
    }

    pub fn parse(
        &self,
        text: &str,
        reg: &RegType,
    ) -> Option<DValue> {
        match reg {
            RegType::U16 => {
                Some(
                    DValue::U16(
                        self.parse_u64(text)? as u16
                    )
                )
            }
            RegType::I16 => {
                Some(
                    DValue::I16(
                        self.parse_i16(text)?
                    )
                )
            }
            RegType::U32 => {
                Some(
                    DValue::U32(
                        self.parse_u64(text)? as u32
                    )
                )
            }
            RegType::I32 => {
                Some(
                    DValue::I32(
                        self.parse_i32(text)?
                    )
                )
            }

            RegType::F32 => {
                Some(
                    DValue::F32(
                        self.parse_f32(text)?
                    )
                )
            }

            RegType::U64 => {
                Some(
                    DValue::U64(
                        self.parse_u64(text)?
                    )
                )
            }

            RegType::I64 => {
                Some(
                    DValue::I64(
                        self.parse_i64(text)?
                    )
                )
            }

            RegType::F64 => {
                Some(
                    DValue::F64(
                        self.parse_f64(text)?
                    )
                )
            }
           
        }
    }

    fn parse_i16(
        &self,
        text: &str,
    ) -> Option<i16> {
        match self {
            DisplayFormat::Decimal => {
                text.trim().parse::<i16>().ok()
            }
            _ => {
                Some(
                    self.parse_u64(text)?
                        as u16
                        as i16
                )
            }
        }
    }

    fn parse_i32(
        &self,
        text: &str,
    ) -> Option<i32> {
        match self {
            DisplayFormat::Decimal => {
                text.trim().parse::<i32>().ok()
            }
            _ => {
                Some(
                    self.parse_u64(text)?
                        as u32
                        as i32
                )
            }
        }
    }

    fn parse_f32(
        &self,
        text: &str,
    ) -> Option<f32> {
        match self {
            DisplayFormat::Decimal => {
                text.trim().parse::<f32>().ok()
            }
            _ => {
                Some(f32::from_bits(
                    self.parse_u64(text)? as u32
                ))
            }
        }
    }
    
    fn parse_i64(
        &self,
        text: &str,
    ) -> Option<i64> {
        match self {
            DisplayFormat::Decimal => {
                text.trim().parse::<i64>().ok()
            }
            _ => {
                Some(
                    self.parse_u64(text)?
                        as i64
                )
            }
        }
    }

    fn parse_u64(
        &self,
        text: &str,
    ) -> Option<u64> {
        let t = text
            .trim()
            .replace('_', "");

        match self {
            DisplayFormat::Binary => {
                let s = t
                    .strip_prefix("0b")
                    .unwrap_or(&t);

                u64::from_str_radix(s, 2).ok()
            }


            DisplayFormat::Octal => {
                let s = t
                    .strip_prefix("0o")
                    .unwrap_or(&t);

                u64::from_str_radix(s, 8).ok()
            }


            DisplayFormat::Decimal => {
                t.parse::<u64>().ok()
            }


            DisplayFormat::Hexadecimal => {
                let s = t
                    .strip_prefix("0x")
                    .unwrap_or(&t);

                u64::from_str_radix(s, 16).ok()
            }
        }
    }

    fn parse_f64(
        &self,
        text: &str,
    ) -> Option<f64> {
        match self {
            DisplayFormat::Decimal => {
                text.trim().parse::<f64>().ok()
            }
            _ => {
                Some(f64::from_bits(
                    self.parse_u64(text)?
                ))
            }
        }
    }

}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum RegType {
    U16,
    #[default]
    I16,
    U32,
    I32,
    F32,
    U64,
    I64,
    F64,
}

impl RegType {
    pub fn span(
        &self, 
    ) -> usize {
        match self {
            RegType::U16 | RegType::I16 => 1,
            RegType::U32 | RegType::I32 | RegType::F32 => 2,
            RegType::U64 | RegType::I64 | RegType::F64 => 4,
        }
    }
}

pub enum DValue {
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    U64(u64),
    I64(i64),
    F64(f64),
}

pub type ShareDataRef = Arc<ShareData>;
pub struct ShareData {
    pub coils: ArcSwap<Vec<bool>>,               //coils原始数据,egui只读
    pub coils_offset: AtomicU16,

    pub discrete_inputs: ArcSwap<Vec<bool>>,       //discrete_inputs原始数据,egui只读
    pub discrete_inputs_offset: AtomicU16,

    pub input_registers: ArcSwap<Vec<u16>>,                     //input_registers原始数据,egui只读
    pub input_registers_offset: AtomicU16,
    pub input_registers_config: ArcSwap<Vec<Option<RegType>>>,  // 输入寄存器数据类型
    pub input_registers_display_format: ArcSwap<Vec<Option<DisplayFormat>>>,    // 输入寄存器显示格式

    pub holding_registers: ArcSwap<Vec<u16>>,       //holding_registers
    pub holding_registers_offset: AtomicU16,
    pub holding_registers_config: ArcSwap<Vec<Option<RegType>>>,
    pub holding_registers_display_format: ArcSwap<Vec<Option<DisplayFormat>>>,

    pub word_swap: AtomicBool,

    pub byte_swap: AtomicBool,
}

impl ShareData {
    pub fn new() -> Self {
        Self {
            coils: ArcSwap::from_pointee(vec![false; 16 as usize]),
            coils_offset: AtomicU16::new(0),

            discrete_inputs: ArcSwap::from_pointee(vec![false; 16 as usize]),
            discrete_inputs_offset: AtomicU16::new(0),

            input_registers: ArcSwap::from_pointee(vec![0; 16 as usize]),
            input_registers_offset: AtomicU16::new(0),
            input_registers_config: ArcSwap::from_pointee(vec![Some(RegType::I16); 16 as usize]),
            input_registers_display_format: ArcSwap::from_pointee(vec![Some(DisplayFormat::Decimal); 16 as usize]),

            holding_registers: ArcSwap::from_pointee(vec![0; 16 as usize]),
            holding_registers_offset: AtomicU16::new(0),
            holding_registers_config: ArcSwap::from_pointee(vec![Some(RegType::I16); 16 as usize]),
            holding_registers_display_format: ArcSwap::from_pointee(vec![Some(DisplayFormat::Decimal); 16 as usize]),
            
            word_swap: AtomicBool::new(false),
            byte_swap: AtomicBool::new(false),
        }
    }


    /// 重置线圈数量
    pub fn resize_coils(
        &self,
        coils_count: u16,
    ) {
        Self::resize_vec(
            &self.coils, 
            coils_count as usize,
            false,
        );

    }

    /// 重置离散输入数量
    pub fn resize_discrete_inputs(
        &self,
        discrete_inputs_count: u16,
    ) {
        Self::resize_vec(
            &self.discrete_inputs, 
            discrete_inputs_count as usize,
            false,
        );
    }

    /// 重置输入寄存器数量
    pub fn resize_input_registers(
        &self,
        input_registers_count: u16,
    ) {
        Self::resize_vec(
            &self.input_registers, 
            input_registers_count as usize,
            0,
        );
        Self::resize_vec(
            &self.input_registers_config,
            input_registers_count as usize,
            Some(RegType::default()),
        );
        
        Self::resize_vec(
            &self.input_registers_display_format,
            input_registers_count as usize,
            Some(DisplayFormat::default()),
        );
    }

    /// 重置保持寄存器数量
    pub fn resize_holding_registers(
        &self,
        holding_registers_count: u16,
    ) {
        Self::resize_vec(
            &self.holding_registers, 
            holding_registers_count as usize,
            0,
        );
        Self::resize_vec(
            &self.holding_registers_config,
            holding_registers_count as usize,
            Some(RegType::default()),
        );
        Self::resize_vec(
            &self.holding_registers_display_format,
            holding_registers_count as usize,
            Some(DisplayFormat::default()),
        );
    }

    fn resize_vec<T: Clone>(
        vec: &ArcSwap<Vec<T>>,
        count: usize,
        default_value: T,
    ) {
        vec.store(Arc::new(vec![default_value; count]));
    }

    /// Read 16-bit raw data
    fn read_16_raw(
        &self,
        in_reg: bool,
        addr: usize,
    ) -> u16 {
        let a = if in_reg {
            self.input_registers.load().get(addr).unwrap().clone()
        } else {
            self.holding_registers.load().get(addr).unwrap().clone()
        };
        let a = if self.byte_swap.load(Ordering::Relaxed) {
            a.swap_bytes()
        } else {
            a
        };
        a
    }

    /// Read 32-bit raw data
    fn read_32_raw(
        &self,
        in_reg: bool,
        addr: usize,
    ) -> u32 {
        let (a, b) = if in_reg {
            (
                self.input_registers.load().get(addr).unwrap().clone(),
                self.input_registers.load().get(addr + 1).unwrap().clone(),
            )
        } else {
            (
                self.holding_registers.load().get(addr).unwrap().clone(),
                self.holding_registers.load().get(addr + 1).unwrap().clone(),
            )
        };
        let (a, b) = if self.word_swap.load(Ordering::Relaxed) {
            (b, a)
        } else {
            (a, b)
        };
        let (a, b) = if self.byte_swap.load(Ordering::Relaxed) {
            (a.swap_bytes(), b.swap_bytes())
        } else {
            (a, b)
        };
        a as u32 | (b as u32) << 16
    }

    /// Read 64-bit raw data
    fn read_64_raw(
        &self,
        in_reg: bool,
        addr: usize,
    ) -> u64 {
        let (a, b, c, d) = if in_reg {
            (
                self.input_registers.load().get(addr).unwrap().clone(),
                self.input_registers.load().get(addr + 1).unwrap().clone(),
                self.input_registers.load().get(addr + 2).unwrap().clone(),
                self.input_registers.load().get(addr + 3).unwrap().clone(),
            )
        } else {
            (
                self.holding_registers.load().get(addr).unwrap().clone(),
                self.holding_registers.load().get(addr + 1).unwrap().clone(),
                self.holding_registers.load().get(addr + 2).unwrap().clone(),
                self.holding_registers.load().get(addr + 3).unwrap().clone(),
            )
        };
        let (a, b, c, d) = if self.word_swap.load(Ordering::Relaxed) {
            (d, c, b, a)
        } else {
            (a, b, c, d)
        };
        let (a, b, c, d) = if self.byte_swap.load(Ordering::Relaxed) {
            (a.swap_bytes(), b.swap_bytes(), c.swap_bytes(), d.swap_bytes())
        } else {
            (a, b, c, d)
        };
        a as u64 | ((b as u64) << 16) | ((c as u64) << 32) | ((d as u64) << 48)
    }
    /// Read registers
    pub fn read_registers(
        &self,
        in_reg: bool,
        addr: usize,
    ) -> Option<String> {
        let (data, reg_type, fmt) = if in_reg {
            (
                self.input_registers.load(),
                self.input_registers_config.load(),
                self.input_registers_display_format.load(),
            )
        } else {
            (
                self.holding_registers.load(),
                self.holding_registers_config.load(),
                self.holding_registers_display_format.load(),
            )
        };
        let reg_type = reg_type.get(addr)?.as_ref()?;
        let fmt = fmt.get(addr)?.as_ref()?;
        if addr + reg_type.span() > data.len() {
            return None;
        }

        let dvalue = match reg_type {
            RegType::U16 => {
                DValue::U16(self.read_16_raw(in_reg, addr))
            }
            RegType::I16 => {
                DValue::I16(self.read_16_raw(in_reg, addr) as i16)
            }
            RegType::U32 => {
                DValue::U32(self.read_32_raw(in_reg, addr))
            }
            RegType::I32 => {
                DValue::I32(self.read_32_raw(in_reg, addr) as i32)
            }
            RegType::F32 => {
                DValue::F32(f32::from_bits(self.read_32_raw(in_reg, addr)))
            }
            RegType::U64 => {
                DValue::U64(self.read_64_raw(in_reg, addr))
            }
            RegType::I64 => {
                DValue::I64(self.read_64_raw(in_reg, addr) as i64)
            }
            RegType::F64 => {
                DValue::F64(f64::from_bits(self.read_64_raw(in_reg, addr)))
            }
        };
        Some(fmt.format(&dvalue))
    }

    /// Write reg_type
    pub fn write_reg_type(
        &self,
        in_reg: bool,
        addr: usize,
        reg_type: RegType,
    ) -> bool {
        let swap = if in_reg {
            &self.input_registers_config
        } else {
            &self.holding_registers_config
        };

        // load snapshot
        let current = swap.load();
        let mut new_config = (**current).clone();
        
        let span = reg_type.span();
        if addr + span > new_config.len() {
            return false;
        }
        // get old configuration
        let old_cfg = match new_config.get(addr).unwrap() {
            Some(v) => v.clone(),
            None => {
                return false;
            }
        };
        let old_span = old_cfg.span();
        new_config[addr] = Some(reg_type);

        if span < old_span {
            for i in addr + span..addr + old_span {
                new_config[i] = Some(RegType::default());
            }
        } else if span > old_span {
            for i in addr + old_span..addr + span {
                new_config[i] = None;
            }
        }

        swap.store(Arc::new(new_config));
        true
    }
    
    /// Write display format
    pub  fn write_display_format(
        &self,
        in_reg: bool,
        addr: usize,
        format: DisplayFormat,
    ) -> bool {
        let swap = if in_reg {
            &self.input_registers_display_format
        } else {
            &self.holding_registers_display_format
        };
        // load snapshot
        let current = swap.load();
        let mut new_format = (**current).clone();
        if addr >= new_format.len() {
            return false;
        }
        new_format[addr] = Some(format);

        swap.store(Arc::new(new_format));

        true
    }

    /// parse 16-bit raw data to Vec<u16>
    fn parse_16_raw(
        &self,
        value: u16,
    ) -> Vec<u16> {
        let a = if self.byte_swap.load(Ordering::Relaxed) {
            value.swap_bytes()
        } else {
            value
        };
        vec![a]
    }
    /// parse 32-bit raw data to Vec<u16>
    fn parse_32_raw(
        &self,
        value: u32,
    ) -> Vec<u16> {
        let (a, b) = {
            (value as u16, (value >> 16) as u16)
        };
        let (a, b) = if self.byte_swap.load(Ordering::Relaxed) {
            (a.swap_bytes(), b.swap_bytes())
        } else {
            (a, b)
        };
        let (a, b) = if self.word_swap.load(Ordering::Relaxed) {
            (b, a)
        } else {
            (a, b)
        };
        vec![a, b]
    }
    /// parse 64-bit raw data to Vec<u16>
    fn parse_64_raw(
        &self,
        value: u64,
    ) -> Vec<u16> {
        let (a, b, c, d) = {
            (value as u16, (value >> 16) as u16, (value >> 32) as u16, (value >> 48) as u16)
        };
        let (a, b, c, d) = if self.byte_swap.load(Ordering::Relaxed) {
            (a.swap_bytes(), b.swap_bytes(), c.swap_bytes(), d.swap_bytes())
        } else {
            (a, b, c, d)
        };
        let (a, b, c, d) = if self.word_swap.load(Ordering::Relaxed) {
            (d, c, b, a)
        } else {
            (a, b, c, d)
        };
        vec![a, b, c, d]
    }

    /// String to DValue
    pub fn parse_string_to_dvalue(
        &self,
        addr: usize,
        value: &String,
    ) -> Option<Vec<u16>> { 
        let reg_type = {
            let cfg = self.holding_registers_config.load();
            cfg.get(addr)?.as_ref()?.clone()
        };

        // read format
        let format = {
            let fmt = self.holding_registers_display_format.load();
            fmt.get(addr)?.as_ref()?.clone()
        };
        let dvalue = format.parse(&value, &reg_type);
        let result = match dvalue? {
            DValue::U16(v) => self.parse_16_raw(v),
            DValue::I16(v) => self.parse_16_raw(v as u16),
            DValue::U32(v) => self.parse_32_raw(v),
            DValue::I32(v) => self.parse_32_raw(v as u32),
            DValue::F32(v) => self.parse_32_raw(v.to_bits()),
            DValue::U64(v) => self.parse_64_raw(v),
            DValue::I64(v) => self.parse_64_raw(v as u64),
            DValue::F64(v) => self.parse_64_raw(v.to_bits()),
        };
        Some(result)
    }
}