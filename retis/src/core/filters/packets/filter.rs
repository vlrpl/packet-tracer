//! # FilterPacket
//!
//! Object for packet filtering it implements from_string() and
//! to_bytes(). While the latter is self explainatory, the second
//! takes as input a pcap-filter string that gets converted to a bpf
//! program using libpcap, the resulting output gets then converted to
//! ebpf and returned for being consumed.

use std::mem;

use anyhow::{bail, Result};
use pcap::{Capture, Linktype};

use super::ebpfinsn::{eBpfInsn, MovInfo};

use crate::{
    bindings::packet_filter_uapi,
    core::filters::packets::{
        cbpf::BpfProg,
        ebpf::{eBpfProg, BpfReg},
    },
};

#[derive(Clone)]
pub(crate) struct FilterPacket(eBpfProg);

impl FilterPacket {
    pub(crate) fn from_string_opt(
        fstring: String,
        layer_type: packet_filter_uapi::filter_type,
    ) -> Result<Self> {
        let link_type = match layer_type {
            packet_filter_uapi::L3 => Linktype(12), // DLT_RAW
            packet_filter_uapi::L2 => Linktype::ETHERNET,
            _ => bail!("Unsupported filter type"),
        };

        let bpf_capture = Capture::dead(link_type)?;
        let program = match bpf_capture.compile(fstring.as_str(), true) {
            Ok(program) => program,
            Err(e) => bail!("Could not compile the filter: {e}"),
        };
        let insns = program.get_instructions();
        let filter =
            BpfProg::try_from(unsafe { mem::transmute::<&[pcap::BpfInstruction], &[u8]>(insns) })?;

        let ebpf_filter = eBpfProg::try_from(filter)?;
        if ebpf_filter.len() > packet_filter_uapi::FILTER_MAX_INSNS as usize {
            bail!("Filter exceeds the maximum allowed size.");
        }

        // ebpf_filter.disasm();
        Ok(FilterPacket(ebpf_filter))
    }

    // Generate an empty eBPF filter containing only a single nop
    // instruction.
    pub(crate) fn reject_filter() -> Self {
        let mut ebpf_filter = eBpfProg::new();

        ebpf_filter.add(eBpfInsn::mov32(MovInfo::Imm {
            dst: BpfReg::R0,
            imm: 0_i32,
        }));

        FilterPacket(ebpf_filter)
    }

    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_bytes())
    }
}
