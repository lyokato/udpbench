#include <linux/bpf.h>
#include <linux/filter.h>
#include <linux/unistd.h>
#include <netinet/in.h>
#include <string.h>
#include <unistd.h>

#ifndef ARRAY_SIZE
#define ARRAY_SIZE(arr) (sizeof(arr) / sizeof((arr)[0]))
#endif

int attach_reuseport_cbpf(int fd, uint16_t mod) {
		struct sock_filter code[] = {
		{ BPF_LD  | BPF_W | BPF_ABS, 0, 0, SKF_AD_OFF + SKF_AD_CPU },
		{ BPF_ALU | BPF_MOD | BPF_K, 0, 0, mod },
		{ BPF_RET | BPF_A, 0, 0, 0 },
	};
	struct sock_fprog p = { .len = ARRAY_SIZE(code), .filter = code };
	return setsockopt(fd, SOL_SOCKET, SO_ATTACH_REUSEPORT_CBPF, &p, sizeof(p));
  }

int attach_reuseport_ebpf(int fd, uint16_t mod) {
  static char bpf_log_buf[65535];
  static const char bpf_license[] = "GPL";

  int bpf_fd;
  int ret;
  const struct bpf_insn prog[] = {
    		/* BPF_MOV64_REG(BPF_REG_6, BPF_REG_1) */
		{ BPF_ALU64 | BPF_MOV | BPF_X, BPF_REG_6, BPF_REG_1, 0, 0 },
		/* BPF_LD_ABS(BPF_W, 0) R0 = (uint32_t)skb[0] */
		{ BPF_LD | BPF_ABS | BPF_W, 0, 0, 0, 0 },
		/* BPF_ALU64_IMM(BPF_MOD, BPF_REG_0, mod) */
		{ BPF_ALU64 | BPF_MOD | BPF_K, BPF_REG_0, 0, 0, mod },
		/* BPF_EXIT_INSN() */
		{ BPF_JMP | BPF_EXIT, 0, 0, 0, 0 }
  };

  union bpf_attr attr;
	memset(&attr, 0, sizeof(attr));
	attr.prog_type = BPF_PROG_TYPE_SOCKET_FILTER;
	attr.insn_cnt = ARRAY_SIZE(prog);
	attr.insns = (unsigned long) &prog;
	attr.license = (unsigned long) &bpf_license;
	attr.log_buf = (unsigned long) &bpf_log_buf;
	attr.log_size = sizeof(bpf_log_buf);
	attr.log_level = 1;
	attr.kern_version = 0;

	bpf_fd = syscall(__NR_bpf, BPF_PROG_LOAD, &attr, sizeof(attr));
	if (bpf_fd < 0) {
    return -1;
  }

  ret = setsockopt(fd, SOL_SOCKET, SO_ATTACH_REUSEPORT_EBPF, &bpf_fd,
			sizeof(bpf_fd));

	close(bpf_fd);

  return ret;
}
