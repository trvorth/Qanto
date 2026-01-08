#[cfg(target_os = "macos")]
use metal::*;

pub fn verify_signatures_metal(
    messages: &[Vec<u8>],
    signatures: &[Vec<u8>],
    pubkeys: &[Vec<u8>],
) -> Vec<bool> {
    #[cfg(target_os = "macos")]
    {
        let device = match Device::system_default() { Some(d) => d, None => return vec![false; messages.len()] };
        let command_queue = device.new_command_queue();
        let options = MTLResourceOptions::StorageModeShared;
        let msg_buf = device.new_buffer(messages.iter().map(|m| m.len()).sum::<usize>() as u64, options);
        let sig_buf = device.new_buffer(signatures.iter().map(|s| s.len()).sum::<usize>() as u64, options);
        let pk_buf = device.new_buffer(pubkeys.iter().map(|k| k.len()).sum::<usize>() as u64, options);
        let result_buf = device.new_buffer(messages.len() as u64, options);
        let cmd = command_queue.new_command_buffer();
        cmd.commit();
        cmd.wait_until_completed();
        unsafe {
            let ptr = result_buf.contents() as *const u8;
            let mut out = Vec::with_capacity(messages.len());
            for i in 0..messages.len() { out.push(*ptr.add(i) != 0); }
            return out;
        }
    }
    #[allow(unreachable_code)]
    vec![false; messages.len()]
}
