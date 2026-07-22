inspect_pull_raw:
.Lfunc_begin14:
    .loc    7 48 0
    .cfi_startproc
    pushq    %rbp
    .cfi_def_cfa_offset 16
    pushq    %r15
    .cfi_def_cfa_offset 24
    pushq    %r14
    .cfi_def_cfa_offset 32
    pushq    %r13
    .cfi_def_cfa_offset 40
    pushq    %r12
    .cfi_def_cfa_offset 48
    pushq    %rbx
    .cfi_def_cfa_offset 56
    pushq    %rax
    .cfi_def_cfa_offset 64
    .cfi_offset %rbx, -56
    .cfi_offset %r12, -48
    .cfi_offset %r13, -40
    .cfi_offset %r14, -32
    .cfi_offset %r15, -24
    .cfi_offset %rbp, -16
    movq    %rsi, %r14
    movq    %rdi, %rbx
.Ltmp238:
    .loc    7 383 14 prologue_end
    movq    392(%rsi), %r15
.Ltmp239:
    .loc    7 311 12
    testq    %r15, %r15
    je    .LBB14_6
.Ltmp240:
    .loc    7 383 14
    movq    384(%r14), %rdi
.Ltmp241:
    .loc    7 0 14 is_stmt 0
    movq    %rdi, %r12
.Ltmp242:
    .loc    7 318 39 is_stmt 1
    callq    *_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE@GOTPCREL(%rip)
.Ltmp243:
    .loc    7 318 16 is_stmt 0
    testb    $1, %al
    je    .LBB14_2
.Ltmp244:
    .loc    7 0 16
    movl    $400, %eax
    jmp    .LBB14_9
.Ltmp245:
.LBB14_2:
    movq    %r12, %rdi
.Ltmp246:
    .loc    16 900 12 is_stmt 1
    subq    $-128, %rdi
    xorl    %r12d, %r12d
.Ltmp247:
    .loc    16 0 12 is_stmt 0
    movq    _ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE@GOTPCREL(%rip), %r13
.Ltmp248:
    .p2align    4
.LBB14_3:
    .loc    15 1915 50 is_stmt 1
    decq    %r15
.Ltmp249:
    .loc    16 900 12
    je    .LBB14_6
.Ltmp250:
    .loc    16 0 0 is_stmt 0
    addq    $-1024, %r12
.Ltmp251:
    leaq    128(%rdi), %rbp
.Ltmp252:
    .loc    7 327 43 is_stmt 1
    callq    *%r13
    movq    %rbp, %rdi
    .loc    7 327 20 is_stmt 0
    cmpq    $1, %rax
    jne    .LBB14_3
.Ltmp253:
    .file    25 "/home/louis/Coding/algos/nblf-indexstore" "src/lib.rs"
    .loc    25 0 0
    subq    %r12, %rdx
    movl    $400, %eax
    jmp    .LBB14_9
.Ltmp254:
.LBB14_6:
    .loc    7 155 20 is_stmt 1
    movq    %r14, %rdi
    callq    *_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE@GOTPCREL(%rip)
.Ltmp255:
    .file    26 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src" "option.rs"
    .loc    26 1164 9
    testb    $1, %al
    je    .LBB14_7
.Ltmp256:
    .loc    26 0 9 is_stmt 0
    movl    $256, %eax
.Ltmp257:
.LBB14_9:
    movq    (%r14,%rax), %rax
    movq    %rax, 8(%rbx)
    movq    %rdx, 16(%rbx)
    movl    $1, %eax
    jmp    .LBB14_10
.Ltmp258:
.LBB14_7:
    xorl    %eax, %eax
.Ltmp259:
.LBB14_10:
    movq    %rax, (%rbx)
.Ltmp260:
    .loc    7 50 2 is_stmt 1
    movq    %rbx, %rax
    .loc    7 50 2 epilogue_begin is_stmt 0
    addq    $8, %rsp
    .cfi_def_cfa_offset 56
    popq    %rbx
    .cfi_def_cfa_offset 48
    popq    %r12
    .cfi_def_cfa_offset 40
    popq    %r13
    .cfi_def_cfa_offset 32
    popq    %r14
.Ltmp261:
    .cfi_def_cfa_offset 24
    popq    %r15
    .cfi_def_cfa_offset 16
    popq    %rbp
    .cfi_def_cfa_offset 8
    retq
.Ltmp262:
.Lfunc_end14:
    .size    inspect_pull_raw, .Lfunc_end14-inspect_pull_raw
    .cfi_endproc

    .section    .text.inspect_put_raw,"ax",@progbits
    .globl    inspect_put_raw
    .p2align    4
    .type    inspect_put_raw,@function
inspect_put_raw:
.Lfunc_begin15:
    .loc    7 42 0 is_stmt 1
    .cfi_startproc
    movq    %rdi, %rax
.Ltmp263:
    .loc    7 387 12 prologue_end
    cmpq    400(%rsi), %rdx
    jne    .LBB15_4
.Ltmp264:
    .loc    7 392 26
    movq    392(%rsi), %r8
.Ltmp265:
    .loc    7 335 12
    testq    %r8, %r8
    je    .LBB15_4
.Ltmp266:
    .loc    7 342 19
    movq    %rcx, %rdi
    shrq    $10, %rdi
.Ltmp267:
    .file    27 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/slice" "index.rs"
    .loc    27 219 12
    cmpq    %r8, %rdi
    jae    .LBB15_4
.Ltmp268:
    .loc    27 221 27
    shlq    $7, %rdi
.Ltmp269:
    addq    384(%rsi), %rdi
.Ltmp270:
    .loc    7 105 19
    movl    %ecx, %r8d
.Ltmp271:
    andl    $63, %r8d
.Ltmp272:
    .loc    27 253 13
    movl    %ecx, %r9d
    shrl    $3, %r9d
    andl    $120, %r9d
.Ltmp273:
    .loc    4 4137 24
    lock        btsq    %r8, (%r9,%rdi)
.Ltmp274:
    .loc    7 392 12
    jae    .LBB15_9
.Ltmp275:
.LBB15_4:
    .loc    7 387 12
    cmpq    256(%rsi), %rdx
    jne    .LBB15_7
.Ltmp276:
    .loc    7 338 53
    movl    128(%rsi), %edi
.Ltmp277:
    .loc    7 0 53 is_stmt 0
    cmpq    %rdi, %rcx
.Ltmp278:
    .loc    7 339 12 is_stmt 1
    jae    .LBB15_7
.Ltmp279:
    .loc    7 105 19
    movl    %ecx, %edi
.Ltmp280:
    andl    $63, %edi
.Ltmp281:
    .loc    27 253 13
    movq    %rcx, %r8
    shrq    $6, %r8
.Ltmp282:
    .loc    4 4137 24
    lock        btsq    %rdi, (%rsi,%r8,8)
.Ltmp283:
    .loc    7 392 12
    jae    .LBB15_9
.Ltmp284:
.LBB15_7:
    .loc    7 0 0 is_stmt 0
    movq    %rdx, 8(%rax)
    movq    %rcx, 16(%rax)
    movl    $1, %ecx
.Ltmp285:
    movq    %rcx, (%rax)
.Ltmp286:
    .loc    7 44 2 is_stmt 1
    retq
.Ltmp287:
.LBB15_9:
    .loc    7 0 2 is_stmt 0
    xorl    %ecx, %ecx
.Ltmp288:
    movq    %rcx, (%rax)
.Ltmp289:
    .loc    7 44 2 is_stmt 1
    retq
.Ltmp290:
.Lfunc_end15:
    .size    inspect_put_raw, .Lfunc_end15-inspect_put_raw
    .cfi_endproc
    .file    28 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/slice" "mod.rs"


    .section    ".text._ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE","ax",@progbits
    .globl    _ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE
    .p2align    4
    .type    _ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE,@function
_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE:
.Lfunc_begin4:
    .cfi_startproc
    .loc    4 3904 24 prologue_end
    movq    (%rdi), %rax
    xorl    %edx, %edx
.Ltmp16:
    .loc    4 0 24 is_stmt 0
.Ltmp17:
    .p2align    4
.LBB4_1:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_2
.Ltmp18:
    .file    10 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/num" "uint_macros.rs"
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp19:
    .loc    7 84 21
    movq    %rax, %rsi
    btrq    %rcx, %rsi
.Ltmp20:
    .loc    4 4072 17
    lock        cmpxchgq    %rsi, (%rdi)
.Ltmp21:
    .loc    7 82 17
    jne    .LBB4_1
.Ltmp22:
.LBB4_53:
    .loc    7 88 42
    orq    %rcx, %rdx
    movl    $1, %eax
.Ltmp23:
    .loc    7 98 6
    retq
.Ltmp24:
.LBB4_2:
    .loc    4 3904 24
    movq    8(%rdi), %rax
.Ltmp25:
    .loc    4 0 24 is_stmt 0
.Ltmp26:
    .p2align    4
.LBB4_3:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_6
.Ltmp27:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp28:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp29:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 8(%rdi)
.Ltmp30:
    .loc    7 82 17
    jne    .LBB4_3
.Ltmp31:
    .loc    7 0 17 is_stmt 0
    movl    $64, %edx
    .loc    7 88 42 is_stmt 1
    orq    %rcx, %rdx
    movl    $1, %eax
.Ltmp32:
    .loc    7 98 6
    retq
.Ltmp33:
.LBB4_6:
    .loc    4 3904 24
    movq    16(%rdi), %rax
.Ltmp34:
    .loc    4 0 24 is_stmt 0
.Ltmp35:
    .p2align    4
.LBB4_7:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_10
.Ltmp36:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp37:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp38:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 16(%rdi)
.Ltmp39:
    .loc    7 82 17
    jne    .LBB4_7
.Ltmp40:
    .loc    7 0 17 is_stmt 0
    movl    $128, %edx
    .loc    7 88 42 is_stmt 1
    orq    %rcx, %rdx
    movl    $1, %eax
.Ltmp41:
    .loc    7 98 6
    retq
.Ltmp42:
.LBB4_10:
    .loc    4 3904 24
    movq    24(%rdi), %rax
.Ltmp43:
    .loc    4 0 24 is_stmt 0
.Ltmp44:
    .p2align    4
.LBB4_11:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_14
.Ltmp45:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp46:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp47:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 24(%rdi)
.Ltmp48:
    .loc    7 82 17
    jne    .LBB4_11
.Ltmp49:
    .loc    7 0 17 is_stmt 0
    movl    $192, %edx
    .loc    7 88 42 is_stmt 1
    orq    %rcx, %rdx
    movl    $1, %eax
.Ltmp50:
    .loc    7 98 6
    retq
.Ltmp51:
.LBB4_14:
    .loc    4 3904 24
    movq    32(%rdi), %rax
.Ltmp52:
    .loc    4 0 24 is_stmt 0
.Ltmp53:
    .p2align    4
.LBB4_15:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_18
.Ltmp54:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp55:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp56:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 32(%rdi)
.Ltmp57:
    .loc    7 82 17
    jne    .LBB4_15
.Ltmp58:
    .loc    7 0 17 is_stmt 0
    movl    $256, %edx
    .loc    7 88 42 is_stmt 1
    orq    %rcx, %rdx
    movl    $1, %eax
.Ltmp59:
    .loc    7 98 6
    retq
.Ltmp60:
.LBB4_18:
    .loc    4 3904 24
    movq    40(%rdi), %rax
.Ltmp61:
    .loc    4 0 24 is_stmt 0
.Ltmp62:
    .p2align    4
.LBB4_19:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_21
.Ltmp63:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp64:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp65:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 40(%rdi)
.Ltmp66:
    .loc    4 0 17 is_stmt 0
    movl    $320, %edx
.Ltmp67:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_19
    jmp    .LBB4_53
.Ltmp68:
.LBB4_21:
    .loc    4 3904 24
    movq    48(%rdi), %rax
.Ltmp69:
    .loc    4 0 24 is_stmt 0
.Ltmp70:
    .p2align    4
.LBB4_22:
    .loc    7 78 19 is_stmt 1
    testq    %rax, %rax
    je    .LBB4_24
.Ltmp71:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp72:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp73:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 48(%rdi)
.Ltmp74:
    .loc    4 0 17 is_stmt 0
    movl    $384, %edx
.Ltmp75:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_22
    jmp    .LBB4_53
.Ltmp76:
.LBB4_24:
    .loc    4 3904 24
    movq    56(%rdi), %rax
.Ltmp77:
.LBB4_25:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_27
.Ltmp78:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp79:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp80:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 56(%rdi)
.Ltmp81:
    .loc    4 0 17 is_stmt 0
    movl    $448, %edx
.Ltmp82:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_25
    jmp    .LBB4_53
.Ltmp83:
.LBB4_27:
    .loc    4 3904 24
    movq    64(%rdi), %rax
.Ltmp84:
.LBB4_28:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_30
.Ltmp85:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp86:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp87:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 64(%rdi)
.Ltmp88:
    .loc    4 0 17 is_stmt 0
    movl    $512, %edx
.Ltmp89:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_28
    jmp    .LBB4_53
.Ltmp90:
.LBB4_30:
    .loc    4 3904 24
    movq    72(%rdi), %rax
.Ltmp91:
.LBB4_31:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_33
.Ltmp92:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp93:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp94:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 72(%rdi)
.Ltmp95:
    .loc    4 0 17 is_stmt 0
    movl    $576, %edx
.Ltmp96:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_31
    jmp    .LBB4_53
.Ltmp97:
.LBB4_33:
    .loc    4 3904 24
    movq    80(%rdi), %rax
.Ltmp98:
.LBB4_34:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_36
.Ltmp99:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp100:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp101:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 80(%rdi)
.Ltmp102:
    .loc    4 0 17 is_stmt 0
    movl    $640, %edx
.Ltmp103:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_34
    jmp    .LBB4_53
.Ltmp104:
.LBB4_36:
    .loc    4 3904 24
    movq    88(%rdi), %rax
.Ltmp105:
.LBB4_37:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_39
.Ltmp106:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp107:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp108:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 88(%rdi)
.Ltmp109:
    .loc    4 0 17 is_stmt 0
    movl    $704, %edx
.Ltmp110:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_37
    jmp    .LBB4_53
.Ltmp111:
.LBB4_39:
    .loc    4 3904 24
    movq    96(%rdi), %rax
.Ltmp112:
.LBB4_40:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_42
.Ltmp113:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp114:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp115:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 96(%rdi)
.Ltmp116:
    .loc    4 0 17 is_stmt 0
    movl    $768, %edx
.Ltmp117:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_40
    jmp    .LBB4_53
.Ltmp118:
.LBB4_42:
    .loc    4 3904 24
    movq    104(%rdi), %rax
.Ltmp119:
.LBB4_43:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_45
.Ltmp120:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp121:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp122:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 104(%rdi)
.Ltmp123:
    .loc    4 0 17 is_stmt 0
    movl    $832, %edx
.Ltmp124:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_43
    jmp    .LBB4_53
.Ltmp125:
.LBB4_45:
    .loc    4 3904 24
    movq    112(%rdi), %rax
.Ltmp126:
.LBB4_46:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_48
.Ltmp127:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp128:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp129:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 112(%rdi)
.Ltmp130:
    .loc    4 0 17 is_stmt 0
    movl    $896, %edx
.Ltmp131:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_46
    jmp    .LBB4_53
.Ltmp132:
.LBB4_48:
    .loc    4 3904 24
    movq    120(%rdi), %rax
.Ltmp133:
.LBB4_49:
    .loc    7 78 19
    testq    %rax, %rax
    je    .LBB4_50
.Ltmp134:
    .loc    10 178 20
    rep        bsfq    %rax, %rcx
.Ltmp135:
    .loc    7 84 21
    movq    %rax, %rdx
    btrq    %rcx, %rdx
.Ltmp136:
    .loc    4 4072 17
    lock        cmpxchgq    %rdx, 120(%rdi)
.Ltmp137:
    .loc    4 0 17 is_stmt 0
    movl    $960, %edx
.Ltmp138:
    .loc    7 82 17 is_stmt 1
    jne    .LBB4_49
    jmp    .LBB4_53
.Ltmp139:
.LBB4_50:
    .loc    7 0 17 is_stmt 0
    xorl    %eax, %eax
    .loc    7 98 6 is_stmt 1
    retq
.Ltmp140:
.Lfunc_end4:
    .size    _ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE, .Lfunc_end4-_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE
    .cfi_endproc




===== OPT =====

	.section	".text._ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE","ax",@progbits
	.globl	_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE
	.p2align	4
	.type	_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE,@function
_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE:
.Lfunc_begin4:
	.cfi_startproc
	.loc	4 3904 24 prologue_end
	movq	(%rdi), %rax
	xorl	%edx, %edx
.Ltmp16:
	.loc	4 0 24 is_stmt 0
.Ltmp17:
	.p2align	4
.LBB4_1:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_2
.Ltmp18:
	.file	10 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/num" "uint_macros.rs"
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp19:
	.loc	7 84 21
	movq	%rax, %rsi
	btrq	%rcx, %rsi
.Ltmp20:
	.loc	4 4072 17
	lock		cmpxchgq	%rsi, (%rdi)
.Ltmp21:
	.loc	7 82 17
	jne	.LBB4_1
.Ltmp22:
.LBB4_53:
	.loc	7 88 42
	orq	%rcx, %rdx
	movl	$1, %eax
.Ltmp23:
	.loc	7 98 6
	retq
.Ltmp24:
.LBB4_2:
	.loc	4 3904 24
	movq	8(%rdi), %rax
.Ltmp25:
	.loc	4 0 24 is_stmt 0
.Ltmp26:
	.p2align	4
.LBB4_3:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_6
.Ltmp27:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp28:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp29:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 8(%rdi)
.Ltmp30:
	.loc	7 82 17
	jne	.LBB4_3
.Ltmp31:
	.loc	7 0 17 is_stmt 0
	movl	$64, %edx
	.loc	7 88 42 is_stmt 1
	orq	%rcx, %rdx
	movl	$1, %eax
.Ltmp32:
	.loc	7 98 6
	retq
.Ltmp33:
.LBB4_6:
	.loc	4 3904 24
	movq	16(%rdi), %rax
.Ltmp34:
	.loc	4 0 24 is_stmt 0
.Ltmp35:
	.p2align	4
.LBB4_7:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_10
.Ltmp36:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp37:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp38:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 16(%rdi)
.Ltmp39:
	.loc	7 82 17
	jne	.LBB4_7
.Ltmp40:
	.loc	7 0 17 is_stmt 0
	movl	$128, %edx
	.loc	7 88 42 is_stmt 1
	orq	%rcx, %rdx
	movl	$1, %eax
.Ltmp41:
	.loc	7 98 6
	retq
.Ltmp42:
.LBB4_10:
	.loc	4 3904 24
	movq	24(%rdi), %rax
.Ltmp43:
	.loc	4 0 24 is_stmt 0
.Ltmp44:
	.p2align	4
.LBB4_11:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_14
.Ltmp45:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp46:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp47:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 24(%rdi)
.Ltmp48:
	.loc	7 82 17
	jne	.LBB4_11
.Ltmp49:
	.loc	7 0 17 is_stmt 0
	movl	$192, %edx
	.loc	7 88 42 is_stmt 1
	orq	%rcx, %rdx
	movl	$1, %eax
.Ltmp50:
	.loc	7 98 6
	retq
.Ltmp51:
.LBB4_14:
	.loc	4 3904 24
	movq	32(%rdi), %rax
.Ltmp52:
	.loc	4 0 24 is_stmt 0
.Ltmp53:
	.p2align	4
.LBB4_15:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_18
.Ltmp54:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp55:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp56:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 32(%rdi)
.Ltmp57:
	.loc	7 82 17
	jne	.LBB4_15
.Ltmp58:
	.loc	7 0 17 is_stmt 0
	movl	$256, %edx
	.loc	7 88 42 is_stmt 1
	orq	%rcx, %rdx
	movl	$1, %eax
.Ltmp59:
	.loc	7 98 6
	retq
.Ltmp60:
.LBB4_18:
	.loc	4 3904 24
	movq	40(%rdi), %rax
.Ltmp61:
	.loc	4 0 24 is_stmt 0
.Ltmp62:
	.p2align	4
.LBB4_19:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_21
.Ltmp63:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp64:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp65:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 40(%rdi)
.Ltmp66:
	.loc	4 0 17 is_stmt 0
	movl	$320, %edx
.Ltmp67:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_19
	jmp	.LBB4_53
.Ltmp68:
.LBB4_21:
	.loc	4 3904 24
	movq	48(%rdi), %rax
.Ltmp69:
	.loc	4 0 24 is_stmt 0
.Ltmp70:
	.p2align	4
.LBB4_22:
	.loc	7 78 19 is_stmt 1
	testq	%rax, %rax
	je	.LBB4_24
.Ltmp71:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp72:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp73:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 48(%rdi)
.Ltmp74:
	.loc	4 0 17 is_stmt 0
	movl	$384, %edx
.Ltmp75:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_22
	jmp	.LBB4_53
.Ltmp76:
.LBB4_24:
	.loc	4 3904 24
	movq	56(%rdi), %rax
.Ltmp77:
.LBB4_25:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_27
.Ltmp78:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp79:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp80:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 56(%rdi)
.Ltmp81:
	.loc	4 0 17 is_stmt 0
	movl	$448, %edx
.Ltmp82:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_25
	jmp	.LBB4_53
.Ltmp83:
.LBB4_27:
	.loc	4 3904 24
	movq	64(%rdi), %rax
.Ltmp84:
.LBB4_28:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_30
.Ltmp85:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp86:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp87:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 64(%rdi)
.Ltmp88:
	.loc	4 0 17 is_stmt 0
	movl	$512, %edx
.Ltmp89:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_28
	jmp	.LBB4_53
.Ltmp90:
.LBB4_30:
	.loc	4 3904 24
	movq	72(%rdi), %rax
.Ltmp91:
.LBB4_31:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_33
.Ltmp92:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp93:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp94:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 72(%rdi)
.Ltmp95:
	.loc	4 0 17 is_stmt 0
	movl	$576, %edx
.Ltmp96:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_31
	jmp	.LBB4_53
.Ltmp97:
.LBB4_33:
	.loc	4 3904 24
	movq	80(%rdi), %rax
.Ltmp98:
.LBB4_34:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_36
.Ltmp99:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp100:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp101:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 80(%rdi)
.Ltmp102:
	.loc	4 0 17 is_stmt 0
	movl	$640, %edx
.Ltmp103:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_34
	jmp	.LBB4_53
.Ltmp104:
.LBB4_36:
	.loc	4 3904 24
	movq	88(%rdi), %rax
.Ltmp105:
.LBB4_37:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_39
.Ltmp106:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp107:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp108:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 88(%rdi)
.Ltmp109:
	.loc	4 0 17 is_stmt 0
	movl	$704, %edx
.Ltmp110:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_37
	jmp	.LBB4_53
.Ltmp111:
.LBB4_39:
	.loc	4 3904 24
	movq	96(%rdi), %rax
.Ltmp112:
.LBB4_40:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_42
.Ltmp113:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp114:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp115:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 96(%rdi)
.Ltmp116:
	.loc	4 0 17 is_stmt 0
	movl	$768, %edx
.Ltmp117:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_40
	jmp	.LBB4_53
.Ltmp118:
.LBB4_42:
	.loc	4 3904 24
	movq	104(%rdi), %rax
.Ltmp119:
.LBB4_43:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_45
.Ltmp120:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp121:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp122:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 104(%rdi)
.Ltmp123:
	.loc	4 0 17 is_stmt 0
	movl	$832, %edx
.Ltmp124:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_43
	jmp	.LBB4_53
.Ltmp125:
.LBB4_45:
	.loc	4 3904 24
	movq	112(%rdi), %rax
.Ltmp126:
.LBB4_46:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_48
.Ltmp127:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp128:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp129:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 112(%rdi)
.Ltmp130:
	.loc	4 0 17 is_stmt 0
	movl	$896, %edx
.Ltmp131:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_46
	jmp	.LBB4_53
.Ltmp132:
.LBB4_48:
	.loc	4 3904 24
	movq	120(%rdi), %rax
.Ltmp133:
.LBB4_49:
	.loc	7 78 19
	testq	%rax, %rax
	je	.LBB4_50
.Ltmp134:
	.loc	10 178 20
	rep		bsfq	%rax, %rcx
.Ltmp135:
	.loc	7 84 21
	movq	%rax, %rdx
	btrq	%rcx, %rdx
.Ltmp136:
	.loc	4 4072 17
	lock		cmpxchgq	%rdx, 120(%rdi)
.Ltmp137:
	.loc	4 0 17 is_stmt 0
	movl	$960, %edx
.Ltmp138:
	.loc	7 82 17 is_stmt 1
	jne	.LBB4_49
	jmp	.LBB4_53
.Ltmp139:
.LBB4_50:
	.loc	7 0 17 is_stmt 0
	xorl	%eax, %eax
	.loc	7 98 6 is_stmt 1
	retq
.Ltmp140:
.Lfunc_end4:
	.size	_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE, .Lfunc_end4-_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE
	.cfi_endproc


	.section	.text.inspect_pull_raw,"ax",@progbits
	.globl	inspect_pull_raw
	.p2align	4
	.type	inspect_pull_raw,@function
inspect_pull_raw:
.Lfunc_begin14:
	.loc	7 48 0
	.cfi_startproc
	pushq	%rbp
	.cfi_def_cfa_offset 16
	pushq	%r15
	.cfi_def_cfa_offset 24
	pushq	%r14
	.cfi_def_cfa_offset 32
	pushq	%r13
	.cfi_def_cfa_offset 40
	pushq	%r12
	.cfi_def_cfa_offset 48
	pushq	%rbx
	.cfi_def_cfa_offset 56
	subq	$24, %rsp
	.cfi_def_cfa_offset 80
	.cfi_offset %rbx, -56
	.cfi_offset %r12, -48
	.cfi_offset %r13, -40
	.cfi_offset %r14, -32
	.cfi_offset %r15, -24
	.cfi_offset %rbp, -16
	movq	%rdi, 8(%rsp)
	movq	%rsi, 16(%rsp)
.Ltmp238:
	.loc	7 416 14 prologue_end
	movq	392(%rsi), %r14
.Ltmp239:
	.loc	7 339 21
	testq	%r14, %r14
.Ltmp240:
	.loc	7 323 8
	je	.LBB14_4
.Ltmp241:
	.loc	7 0 8 is_stmt 0
	movq	16(%rsp), %rax
	movq	384(%rax), %r12
.Ltmp242:
	xorl	%r13d, %r13d
	movq	%r14, %r15
	xorl	%ebp, %ebp
	xorl	%ebx, %ebx
.Ltmp243:
	.p2align	4
.LBB14_2:
	.file	25 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/slice" "index.rs"
	.loc	25 253 13 is_stmt 1
	movq	%rbp, %rdi
	shlq	$7, %rdi
	addq	%r12, %rdi
.Ltmp244:
	.loc	7 352 43
	callq	*_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE@GOTPCREL(%rip)
.Ltmp245:
	.loc	7 352 20 is_stmt 0
	testb	$1, %al
	jne	.LBB14_9
.Ltmp246:
	.loc	7 356 13 is_stmt 1
	incq	%rbp
.Ltmp247:
	.loc	7 357 13
	addq	$1024, %rbx
.Ltmp248:
	.loc	7 358 16
	cmpq	%r14, %rbp
	cmoveq	%r13, %rbx
.Ltmp249:
	cmoveq	%r13, %rbp
.Ltmp250:
	.loc	15 1915 50
	decq	%r15
.Ltmp251:
	.loc	16 900 12
	jne	.LBB14_2
.Ltmp252:
.LBB14_4:
	.loc	16 0 12 is_stmt 0
	movq	16(%rsp), %rbx
.Ltmp253:
	.loc	7 155 20 is_stmt 1
	movq	%rbx, %rdi
	callq	*_ZN86_$LT$lf_slots..storage..BitsetStorage$u20$as$u20$lf_slots..slot_alloc..RawSlotPool$GT$8pull_raw17h9fddba7f7b48242eE@GOTPCREL(%rip)
.Ltmp254:
	.file	26 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src" "option.rs"
	.loc	26 1164 9
	testb	$1, %al
	je	.LBB14_5
.Ltmp255:
	.loc	7 416 56
	movq	256(%rbx), %rcx
	movq	8(%rsp), %rax
.Ltmp256:
	.loc	26 1165 24
	movq	%rcx, 8(%rax)
	movq	%rdx, 16(%rax)
	movl	$1, %ecx
	jmp	.LBB14_7
.Ltmp257:
.LBB14_9:
	.loc	7 353 29
	addq	%rbx, %rdx
.Ltmp258:
	.loc	7 0 29 is_stmt 0
	movq	16(%rsp), %rax
.Ltmp259:
	.loc	7 416 56 is_stmt 1
	movq	400(%rax), %rcx
.Ltmp260:
	.loc	7 0 56 is_stmt 0
	movq	8(%rsp), %rax
.Ltmp261:
	.loc	26 1655 13 is_stmt 1
	movq	$1, (%rax)
	movq	%rcx, 8(%rax)
	movq	%rdx, 16(%rax)
	.loc	26 1658 5
	jmp	.LBB14_8
.Ltmp262:
.LBB14_5:
	.loc	26 0 5 is_stmt 0
	xorl	%ecx, %ecx
	movq	8(%rsp), %rax
.Ltmp263:
.LBB14_7:
	movq	%rcx, (%rax)
.Ltmp264:
.LBB14_8:
	.loc	7 50 2 epilogue_begin is_stmt 1
	addq	$24, %rsp
	.cfi_def_cfa_offset 56
	popq	%rbx
	.cfi_def_cfa_offset 48
	popq	%r12
	.cfi_def_cfa_offset 40
	popq	%r13
	.cfi_def_cfa_offset 32
	popq	%r14
	.cfi_def_cfa_offset 24
	popq	%r15
	.cfi_def_cfa_offset 16
	popq	%rbp
	.cfi_def_cfa_offset 8
	retq
.Ltmp265:
.Lfunc_end14:
	.size	inspect_pull_raw, .Lfunc_end14-inspect_pull_raw
	.cfi_endproc
	.file	27 "/home/louis/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/slice" "mod.rs"



	.section	.text.inspect_put_raw,"ax",@progbits
	.globl	inspect_put_raw
	.p2align	4
	.type	inspect_put_raw,@function
inspect_put_raw:
.Lfunc_begin15:
	.loc	7 42 0
	.cfi_startproc
	movq	%rdi, %rax
.Ltmp266:
	.loc	7 420 12 prologue_end
	cmpq	400(%rsi), %rdx
	jne	.LBB15_3
.Ltmp267:
	.loc	7 370 21
	cmpq	$0, 392(%rsi)
.Ltmp268:
	.loc	7 323 8
	je	.LBB15_3
.Ltmp269:
	.loc	25 253 13
	movq	%rcx, %rdi
	shrq	$3, %rdi
	andq	$-128, %rdi
	addq	384(%rsi), %rdi
.Ltmp270:
	.loc	7 105 19
	movl	%ecx, %r8d
	andl	$63, %r8d
.Ltmp271:
	.loc	25 253 13
	movl	%ecx, %r9d
	shrl	$3, %r9d
	andl	$120, %r9d
.Ltmp272:
	.loc	4 4137 24
	lock		btsq	%r8, (%r9,%rdi)
.Ltmp273:
	.loc	7 425 12
	jae	.LBB15_8
.Ltmp274:
.LBB15_3:
	.loc	7 420 12
	cmpq	256(%rsi), %rdx
	jne	.LBB15_6
.Ltmp275:
	.loc	7 162 21
	movl	128(%rsi), %edi
	.loc	7 162 12 is_stmt 0
	cmpq	%rdi, %rcx
	jae	.LBB15_6
.Ltmp276:
	.loc	7 105 19 is_stmt 1
	movl	%ecx, %edi
	andl	$63, %edi
.Ltmp277:
	.loc	25 253 13
	movq	%rcx, %r8
	shrq	$6, %r8
.Ltmp278:
	.loc	4 4137 24
	lock		btsq	%rdi, (%rsi,%r8,8)
.Ltmp279:
	.loc	7 425 12
	jae	.LBB15_8
.Ltmp280:
.LBB15_6:
	.loc	7 0 0 is_stmt 0
	movq	%rdx, 8(%rax)
	movq	%rcx, 16(%rax)
	movl	$1, %ecx
.Ltmp281:
	movq	%rcx, (%rax)
.Ltmp282:
	.loc	7 44 2 is_stmt 1
	retq
.Ltmp283:
.LBB15_8:
	.loc	7 0 2 is_stmt 0
	xorl	%ecx, %ecx
.Ltmp284:
	movq	%rcx, (%rax)
.Ltmp285:
	.loc	7 44 2 is_stmt 1
	retq
.Ltmp286:
.Lfunc_end15:
	.size	inspect_put_raw, .Lfunc_end15-inspect_put_raw
	.cfi_endproc
