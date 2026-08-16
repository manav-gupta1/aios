use spin::Lazy;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const KERNEL_STACK_SIZE: usize = 32 * 1024; // 32 KiB

static mut DOUBLE_FAULT_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];
static mut PRIVILEGE_STACK: [u8; KERNEL_STACK_SIZE] = [0; KERNEL_STACK_SIZE];

#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    let ptr = core::ptr::addr_of!(DOUBLE_FAULT_STACK);
    let df_top = VirtAddr::from_ptr(ptr).as_u64() + KERNEL_STACK_SIZE as u64;
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = VirtAddr::new(df_top);

    let priv_ptr = core::ptr::addr_of!(PRIVILEGE_STACK);
    let priv_top = VirtAddr::from_ptr(priv_ptr).as_u64() + KERNEL_STACK_SIZE as u64;
    tss.privilege_stack_table[0] = VirtAddr::new(priv_top);
    tss
});

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let kernel_code = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data = gdt.append(Descriptor::kernel_data_segment());
    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());
    let tss = gdt.append(Descriptor::tss_segment(&TSS));

    (
        gdt,
        Selectors {
            kernel_code,
            kernel_data,
            user_data,
            user_code,
            tss,
        },
    )
});

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.kernel_code);
        DS::set_reg(GDT.1.kernel_data);
        ES::set_reg(GDT.1.kernel_data);
        SS::set_reg(GDT.1.kernel_data);
        load_tss(GDT.1.tss);
    }
}

pub fn get_selectors() -> &'static Selectors {
    &GDT.1
}

pub fn set_privilege_stack(stack_top: VirtAddr) {
    unsafe {
        let tss_ptr = core::ptr::addr_of!(*TSS) as *mut TaskStateSegment;
        (*tss_ptr).privilege_stack_table[0] = stack_top;
    }
}
