initSidebarItems({"constant":[["BOOT_GDT_ENTRY_COUNT",""],["GDT_ENTRY_COUNT",""],["USER_TCB_OFFSET",""]],"fn":[["init","Initialize the actual GDT stored in TLS."],["init_boot","Initialize the bootstrap GDT which is required to initialize TLS (Thread Local Storage) support so, after the kernel heap we will map the TLS section and initialize the actual GDT and then each CPU will have it’s own GDT but we only will have to define it once as a `#[thread_local]`."],["load_cs",""],["load_ds",""],["load_es",""],["load_fs",""],["load_gdt",""],["load_gs",""],["load_ss",""],["load_tss",""]],"static":[["BOOT_GDT",""],["FAULT_STACK",""],["GDT",""],["INIT_STACK",""],["PROCESSOR_CONTROL_REGION",""]],"struct":[["GdtAccessFlags",""],["GdtDescriptor",""],["GdtEntry",""],["GdtEntryFlags",""],["GdtEntryType",""],["ProcessorControlRegion",""],["SegmentSelector","Specifies which element to load into a segment from descriptor tables (i.e., is a index to LDT or GDT table with some additional flags)."],["Tss","The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task."]]});