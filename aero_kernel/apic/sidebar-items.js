window.SIDEBAR_ITEMS = {"constant":[["APIC_SPURIOUS_VECTOR",""],["X2APIC_BASE_MSR",""],["XAPIC_EOI","EOI register. Write-only."],["XAPIC_ESR","Error Status Register (ESR). Read/write."],["XAPIC_ICR","Interrupt Command Register (ICR). Read/write. (64-bit register)"],["XAPIC_ID","Local APIC ID register. Read-only. See Section 10.12.5.1 for initial values."],["XAPIC_LVT_ERROR","LVT Error register. Read/write."],["XAPIC_LVT_TIMER","LVT Timer register. Read/write. See Figure 10-8 for reserved bits."],["XAPIC_SVR","Spurious Interrupt Vector Register (SVR). Read/write."],["XAPIC_TIMER_CURRENT_COUNT","Current Count register (for Timer). Read-only."],["XAPIC_TIMER_DIV_CONF","Divide Configuration Register (DCR; for Timer). Read/write. See Figure 10-10 for reserved bits."],["XAPIC_TIMER_INIT_COUNT","Initial Count register (for Timer). Read/write."],["XAPIC_TPR","Task Priority Register (TPR). Read/write. Bits 31:8 are reserved."]],"enum":[["ApicType",""]],"fn":[["get_bsp_id","Get the local BSP’s id."],["get_cpu_count","Return the number of active CPUs."],["get_local_apic","Get a mutable reference to the local apic."],["init","Initialize the local apic."],["io_apic_from_redirect","Return the index of the I/O APIC that handles this redirect."],["io_apic_get_max_redirect","Get the maximum number of redirects this I/O APIC can handle."],["io_apic_read","Read from the `io_apic_id` I/O APIC as described by the MADT."],["io_apic_set_redirect",""],["io_apic_setup_legacy_irq",""],["io_apic_write","Write from the `io_apic_id` I/O APIC as described by the MADT."],["is_bsp_ready",""],["lapic_error_handler",""],["mark_bsp_ready",""]],"static":[["AP_READY",""],["BSP_APIC_ID",""],["BSP_READY",""],["CPU_COUNT","The count of all the active CPUs."],["LOCAL_APIC",""]],"struct":[["IoApicHeader",""],["LocalApic",""]]};