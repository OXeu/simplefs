
pub trait BlockDevice {
    fn id(&self) -> usize;
    fn read(&self, block: usize, buf: &mut [u8]);
    fn write(&self, block: usize, data: &[u8]);
    fn sync(&self);
}
