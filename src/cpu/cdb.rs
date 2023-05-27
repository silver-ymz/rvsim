use std::fmt::{self, Display};

const CDB_BUFFER_SIZE: usize = 8;

#[derive(Debug, Default)]
pub struct Cdb {
    buffer: [CdbData; CDB_BUFFER_SIZE],
}

#[derive(Debug, Default, Clone, Copy)]
struct CdbData {
    station_id: u8,
    tag: u8, // 0-5: reg_num, 6: clock, 7: valid
    data: u32,
}

impl CdbData {
    pub fn new(station_id: u8, num: u8, data: u32) -> Self {
        CdbData {
            station_id,
            tag: (1 << 7) | num,
            data,
        }
    }

    pub fn get_reg(&self, num: u8) -> Option<u32> {
        if self.tag == (0b11000000 | num) {
            Some(self.data)
        } else {
            None
        }
    }

    pub fn get_station(&self, station_id: u8) -> Option<u32> {
        if self.station_id == station_id && (self.tag & 0b11000000) == 0b11000000 {
            Some(self.data)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tag & 0b10000000 == 0
    }

    pub fn clock(&mut self) {
        if self.is_empty() {
            return;
        }

        if self.tag & 0b01000000 == 0 {
            self.tag |= 0b01000000;
        } else {
            self.tag = 0;
        }
    }
}

impl Cdb {
    pub fn get_reg(&self, num: u8) -> Option<u32> {
        for data in &self.buffer {
            if let Some(data) = data.get_reg(num) {
                return Some(data);
            }
        }
        None
    }

    pub fn get_station(&self, station_id: u8) -> Option<u32> {
        for data in &self.buffer {
            if let Some(data) = data.get_station(station_id) {
                return Some(data);
            }
        }
        None
    }

    pub fn send(&mut self, station: u8, num: u8, data: u32) {
        for cdb_data in &mut self.buffer {
            if cdb_data.is_empty() {
                dbg!("send to cdb");
                *cdb_data = CdbData::new(station, num, data);
                return;
            }
        }

        panic!("CDB buffer is full!");
    }

    pub fn exec(&mut self) {
        for data in &mut self.buffer {
            data.clock();
        }
    }
}

impl Display for CdbData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "station_id: {}, tag: {:08b}, data: {}",
            self.station_id, self.tag, self.data
        )
    }
}

impl Display for Cdb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut empty = true;
        for data in &self.buffer {
            if !data.is_empty() {
                writeln!(f, "{}", data)?;
                empty = false;
            }
        }
        if empty {
            writeln!(f, "empty")?;
        }

        Ok(())
    }
}
