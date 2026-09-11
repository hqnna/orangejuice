//
// The BSD sockets surface: the types, constants and procedures that
// `Socket.jai` does not declare by hand.
//
// Generated from the system's own C headers by `tools/cbind`, the way the
// reference generates its bindings — see `tools/cbind/README.md`.
//

#scope_module

libc :: #system_library "c";

#scope_export

// ------------------------------------------------------------- constants ---

AF_ALG :: 38;
AF_APPLETALK :: 5;
AF_ASH :: 18;
AF_ATMPVC :: 8;
AF_ATMSVC :: 20;
AF_AX25 :: 3;
AF_BLUETOOTH :: 31;
AF_BRIDGE :: 7;
AF_CAIF :: 37;
AF_CAN :: 29;
AF_DECnet :: 12;
AF_ECONET :: 19;
AF_FILE :: 1;
AF_IB :: 27;
AF_IEEE802154 :: 36;
AF_INET :: 2;
AF_INET6 :: 10;
AF_IPX :: 4;
AF_IRDA :: 23;
AF_ISDN :: 34;
AF_IUCV :: 32;
AF_KCM :: 41;
AF_KEY :: 15;
AF_LLC :: 26;
AF_LOCAL :: 1;
AF_MAX :: 46;
AF_MPLS :: 28;
AF_NETBEUI :: 13;
AF_NETLINK :: 16;
AF_NETROM :: 6;
AF_NFC :: 39;
AF_PACKET :: 17;
AF_PHONET :: 35;
AF_PPPOX :: 24;
AF_QIPCRTR :: 42;
AF_RDS :: 21;
AF_ROSE :: 11;
AF_ROUTE :: 16;
AF_RXRPC :: 33;
AF_SECURITY :: 14;
AF_SMC :: 43;
AF_SNA :: 22;
AF_TIPC :: 30;
AF_UNIX :: 1;
AF_UNSPEC :: 0;
AF_VSOCK :: 40;
AF_WANPIPE :: 25;
AF_X25 :: 9;
EAI_ADDRFAMILY :: -9;
EAI_AGAIN :: -3;
EAI_ALLDONE :: -103;
EAI_BADFLAGS :: -1;
EAI_CANCELED :: -101;
EAI_FAIL :: -4;
EAI_FAMILY :: -6;
EAI_IDN_ENCODE :: -105;
EAI_INPROGRESS :: -100;
EAI_INTR :: -104;
EAI_MEMORY :: -10;
EAI_NODATA :: -5;
EAI_NONAME :: -2;
EAI_NOTCANCELED :: -102;
EAI_OVERFLOW :: -12;
EAI_SERVICE :: -8;
EAI_SOCKTYPE :: -7;
EAI_SYSTEM :: -11;
GAI_NOWAIT :: 1;
GAI_WAIT :: 0;
HOST_NOT_FOUND :: 1;
IFHWADDRLEN :: 6;
IFNAMSIZ :: 16;
IF_NAMESIZE :: 16;
INADDR_ALLHOSTS_GROUP :: 3758096385;
INADDR_ALLRTRS_GROUP :: 3758096386;
INADDR_ANY :: 0;
INADDR_BROADCAST :: 4294967295;
INADDR_LOOPBACK :: 2130706433;
INADDR_MAX_LOCAL_GROUP :: 3758096639;
INADDR_NONE :: 4294967295;
INADDR_UNSPEC_GROUP :: 3758096384;
INET6_ADDRSTRLEN :: 46;
INET_ADDRSTRLEN :: 16;
IN_CLASSA_HOST :: 16777215;
IN_CLASSA_MAX :: 128;
IN_CLASSA_NET :: 4278190080;
IN_CLASSA_NSHIFT :: 24;
IN_CLASSB_HOST :: 65535;
IN_CLASSB_MAX :: 65536;
IN_CLASSB_NET :: 4294901760;
IN_CLASSB_NSHIFT :: 16;
IN_CLASSC_HOST :: 255;
IN_CLASSC_NET :: 4294967040;
IN_CLASSC_NSHIFT :: 8;
IN_LOOPBACKNET :: 127;
IPPORT_RESERVED :: 1024;
IPV6_2292DSTOPTS :: 4;
IPV6_2292HOPLIMIT :: 8;
IPV6_2292HOPOPTS :: 3;
IPV6_2292PKTINFO :: 2;
IPV6_2292PKTOPTIONS :: 6;
IPV6_2292RTHDR :: 5;
IPV6_ADDRFORM :: 1;
IPV6_ADDR_PREFERENCES :: 72;
IPV6_ADD_MEMBERSHIP :: 20;
IPV6_AUTHHDR :: 10;
IPV6_AUTOFLOWLABEL :: 70;
IPV6_CHECKSUM :: 7;
IPV6_DONTFRAG :: 62;
IPV6_DROP_MEMBERSHIP :: 21;
IPV6_DSTOPTS :: 59;
IPV6_HDRINCL :: 36;
IPV6_HOPLIMIT :: 52;
IPV6_HOPOPTS :: 54;
IPV6_IPSEC_POLICY :: 34;
IPV6_JOIN_ANYCAST :: 27;
IPV6_JOIN_GROUP :: 20;
IPV6_LEAVE_ANYCAST :: 28;
IPV6_LEAVE_GROUP :: 21;
IPV6_MINHOPCOUNT :: 73;
IPV6_MTU :: 24;
IPV6_MTU_DISCOVER :: 23;
IPV6_MULTICAST_HOPS :: 18;
IPV6_MULTICAST_IF :: 17;
IPV6_MULTICAST_LOOP :: 19;
IPV6_NEXTHOP :: 9;
IPV6_ORIGDSTADDR :: 74;
IPV6_PATHMTU :: 61;
IPV6_PKTINFO :: 50;
IPV6_PMTUDISC_DO :: 2;
IPV6_PMTUDISC_DONT :: 0;
IPV6_PMTUDISC_INTERFACE :: 4;
IPV6_PMTUDISC_OMIT :: 5;
IPV6_PMTUDISC_PROBE :: 3;
IPV6_PMTUDISC_WANT :: 1;
IPV6_RECVDSTOPTS :: 58;
IPV6_RECVERR :: 25;
IPV6_RECVFRAGSIZE :: 77;
IPV6_RECVHOPLIMIT :: 51;
IPV6_RECVHOPOPTS :: 53;
IPV6_RECVORIGDSTADDR :: 74;
IPV6_RECVPATHMTU :: 60;
IPV6_RECVPKTINFO :: 49;
IPV6_RECVRTHDR :: 56;
IPV6_RECVTCLASS :: 66;
IPV6_ROUTER_ALERT :: 22;
IPV6_RTHDR :: 57;
IPV6_RTHDRDSTOPTS :: 55;
IPV6_RTHDR_LOOSE :: 0;
IPV6_RTHDR_STRICT :: 1;
IPV6_RTHDR_TYPE_0 :: 0;
IPV6_RXDSTOPTS :: 59;
IPV6_RXHOPOPTS :: 54;
IPV6_TCLASS :: 67;
IPV6_TRANSPARENT :: 75;
IPV6_UNICAST_HOPS :: 16;
IPV6_UNICAST_IF :: 76;
IPV6_V6ONLY :: 26;
IPV6_XFRM_POLICY :: 35;
IP_ADD_MEMBERSHIP :: 35;
IP_ADD_SOURCE_MEMBERSHIP :: 39;
IP_BIND_ADDRESS_NO_PORT :: 24;
IP_BLOCK_SOURCE :: 38;
IP_CHECKSUM :: 23;
IP_DEFAULT_MULTICAST_LOOP :: 1;
IP_DEFAULT_MULTICAST_TTL :: 1;
IP_DROP_MEMBERSHIP :: 36;
IP_DROP_SOURCE_MEMBERSHIP :: 40;
IP_FREEBIND :: 15;
IP_HDRINCL :: 3;
IP_IPSEC_POLICY :: 16;
IP_MAX_MEMBERSHIPS :: 20;
IP_MINTTL :: 21;
IP_MSFILTER :: 41;
IP_MTU :: 14;
IP_MTU_DISCOVER :: 10;
IP_MULTICAST_ALL :: 49;
IP_MULTICAST_IF :: 32;
IP_MULTICAST_LOOP :: 34;
IP_MULTICAST_TTL :: 33;
IP_NODEFRAG :: 22;
IP_OPTIONS :: 4;
IP_ORIGDSTADDR :: 20;
IP_PASSSEC :: 18;
IP_PKTINFO :: 8;
IP_PKTOPTIONS :: 9;
IP_PMTUDISC :: 10;
IP_PMTUDISC_DO :: 2;
IP_PMTUDISC_DONT :: 0;
IP_PMTUDISC_INTERFACE :: 4;
IP_PMTUDISC_OMIT :: 5;
IP_PMTUDISC_PROBE :: 3;
IP_PMTUDISC_WANT :: 1;
IP_RECVERR :: 11;
IP_RECVFRAGSIZE :: 25;
IP_RECVOPTS :: 6;
IP_RECVORIGDSTADDR :: 20;
IP_RECVRETOPTS :: 7;
IP_RECVTOS :: 13;
IP_RECVTTL :: 12;
IP_RETOPTS :: 7;
IP_ROUTER_ALERT :: 5;
IP_TOS :: 1;
IP_TRANSPARENT :: 19;
IP_TTL :: 2;
IP_UNBLOCK_SOURCE :: 37;
IP_UNICAST_IF :: 50;
IP_XFRM_POLICY :: 17;
MCAST_BLOCK_SOURCE :: 43;
MCAST_EXCLUDE :: 0;
MCAST_INCLUDE :: 1;
MCAST_JOIN_GROUP :: 42;
MCAST_JOIN_SOURCE_GROUP :: 46;
MCAST_LEAVE_GROUP :: 45;
MCAST_LEAVE_SOURCE_GROUP :: 47;
MCAST_MSFILTER :: 48;
MCAST_UNBLOCK_SOURCE :: 44;
NETDB_INTERNAL :: -1;
NETDB_SUCCESS :: 0;
NI_DGRAM :: 16;
NI_IDN :: 32;
NI_IDN_ALLOW_UNASSIGNED :: 64;
NI_IDN_USE_STD3_ASCII_RULES :: 128;
NI_MAXHOST :: 1025;
NI_MAXSERV :: 32;
NI_NAMEREQD :: 8;
NI_NOFQDN :: 4;
NI_NUMERICHOST :: 1;
NI_NUMERICSERV :: 2;
NO_ADDRESS :: 4;
NO_DATA :: 4;
NO_RECOVERY :: 3;
PF_ALG :: 38;
PF_APPLETALK :: 5;
PF_ASH :: 18;
PF_ATMPVC :: 8;
PF_ATMSVC :: 20;
PF_AX25 :: 3;
PF_BLUETOOTH :: 31;
PF_BRIDGE :: 7;
PF_CAIF :: 37;
PF_CAN :: 29;
PF_DECnet :: 12;
PF_ECONET :: 19;
PF_FILE :: 1;
PF_IB :: 27;
PF_IEEE802154 :: 36;
PF_INET :: 2;
PF_INET6 :: 10;
PF_IPX :: 4;
PF_IRDA :: 23;
PF_ISDN :: 34;
PF_IUCV :: 32;
PF_KCM :: 41;
PF_KEY :: 15;
PF_LLC :: 26;
PF_LOCAL :: 1;
PF_MAX :: 46;
PF_MPLS :: 28;
PF_NETBEUI :: 13;
PF_NETLINK :: 16;
PF_NETROM :: 6;
PF_NFC :: 39;
PF_PACKET :: 17;
PF_PHONET :: 35;
PF_PPPOX :: 24;
PF_QIPCRTR :: 42;
PF_RDS :: 21;
PF_ROSE :: 11;
PF_ROUTE :: 16;
PF_RXRPC :: 33;
PF_SECURITY :: 14;
PF_SMC :: 43;
PF_SNA :: 22;
PF_TIPC :: 30;
PF_UNIX :: 1;
PF_UNSPEC :: 0;
PF_VSOCK :: 40;
PF_WANPIPE :: 25;
PF_X25 :: 9;
SOL_AAL :: 265;
SOL_ALG :: 279;
SOL_ATM :: 264;
SOL_BLUETOOTH :: 274;
SOL_CAIF :: 278;
SOL_DCCP :: 269;
SOL_DECNET :: 261;
SOL_ICMPV6 :: 58;
SOL_IP :: 0;
SOL_IPV6 :: 41;
SOL_IRDA :: 266;
SOL_IUCV :: 277;
SOL_KCM :: 281;
SOL_LLC :: 268;
SOL_NETBEUI :: 267;
SOL_NETLINK :: 270;
SOL_NFC :: 280;
SOL_PACKET :: 263;
SOL_PNPIPE :: 275;
SOL_PPPOL2TP :: 273;
SOL_RAW :: 255;
SOL_RDS :: 276;
SOL_RXRPC :: 272;
SOL_SOCKET :: 1;
SOL_TCP :: 6;
SOL_TIPC :: 271;
SOL_TLS :: 282;
SOL_X25 :: 262;
SOMAXCONN :: 4096;
SO_ACCEPTCONN :: 30;
SO_ATTACH_BPF :: 50;
SO_ATTACH_FILTER :: 26;
SO_ATTACH_REUSEPORT_CBPF :: 51;
SO_ATTACH_REUSEPORT_EBPF :: 52;
SO_BINDTODEVICE :: 25;
SO_BPF_EXTENSIONS :: 48;
SO_BROADCAST :: 6;
SO_BSDCOMPAT :: 14;
SO_BUSY_POLL :: 46;
SO_CNX_ADVICE :: 53;
SO_COOKIE :: 57;
SO_DEBUG :: 1;
SO_DETACH_BPF :: 27;
SO_DETACH_FILTER :: 27;
SO_DOMAIN :: 39;
SO_DONTROUTE :: 5;
SO_ERROR :: 4;
SO_GET_FILTER :: 26;
SO_INCOMING_CPU :: 49;
SO_INCOMING_NAPI_ID :: 56;
SO_KEEPALIVE :: 9;
SO_LINGER :: 13;
SO_LOCK_FILTER :: 44;
SO_MARK :: 36;
SO_MAX_PACING_RATE :: 47;
SO_MEMINFO :: 55;
SO_NOFCS :: 43;
SO_NO_CHECK :: 11;
SO_OOBINLINE :: 10;
SO_PASSCRED :: 16;
SO_PASSSEC :: 34;
SO_PEEK_OFF :: 42;
SO_PEERCRED :: 17;
SO_PEERGROUPS :: 59;
SO_PEERNAME :: 28;
SO_PEERSEC :: 31;
SO_PRIORITY :: 12;
SO_PROTOCOL :: 38;
SO_RCVBUF :: 8;
SO_RCVBUFFORCE :: 33;
SO_RCVLOWAT :: 18;
SO_RCVTIMEO :: 20;
SO_REUSEADDR :: 2;
SO_REUSEPORT :: 15;
SO_RXQ_OVFL :: 40;
SO_SECURITY_AUTHENTICATION :: 22;
SO_SECURITY_ENCRYPTION_NETWORK :: 24;
SO_SECURITY_ENCRYPTION_TRANSPORT :: 23;
SO_SELECT_ERR_QUEUE :: 45;
SO_SNDBUF :: 7;
SO_SNDBUFFORCE :: 32;
SO_SNDLOWAT :: 19;
SO_SNDTIMEO :: 21;
SO_TIMESTAMP :: 29;
SO_TIMESTAMPING :: 37;
SO_TIMESTAMPNS :: 35;
SO_TYPE :: 3;
SO_WIFI_STATUS :: 41;
SO_ZEROCOPY :: 60;
TCPI_OPT_ECN :: 8;
TCPI_OPT_ECN_SEEN :: 16;
TCPI_OPT_SACK :: 2;
TCPI_OPT_SYN_DATA :: 32;
TCPI_OPT_TIMESTAMPS :: 1;
TCPI_OPT_WSCALE :: 4;
TCPOLEN_MAXSEG :: 4;
TCPOLEN_SACK_PERMITTED :: 2;
TCPOLEN_TIMESTAMP :: 10;
TCPOLEN_TSTAMP_APPA :: 12;
TCPOLEN_WINDOW :: 3;
TCPOPT_EOL :: 0;
TCPOPT_MAXSEG :: 2;
TCPOPT_NOP :: 1;
TCPOPT_SACK :: 5;
TCPOPT_SACK_PERMITTED :: 4;
TCPOPT_TIMESTAMP :: 8;
TCPOPT_TSTAMP_HDR :: 16844810;
TCPOPT_WINDOW :: 3;
TCP_CC_INFO :: 26;
TCP_CONGESTION :: 13;
TCP_COOKIE_IN_ALWAYS :: 1;
TCP_COOKIE_MAX :: 16;
TCP_COOKIE_MIN :: 8;
TCP_COOKIE_OUT_NEVER :: 2;
TCP_COOKIE_PAIR_SIZE :: 32;
TCP_COOKIE_TRANSACTIONS :: 15;
TCP_CORK :: 3;
TCP_DEFER_ACCEPT :: 9;
TCP_FASTOPEN :: 23;
TCP_FASTOPEN_CONNECT :: 30;
TCP_INFO :: 11;
TCP_KEEPCNT :: 6;
TCP_KEEPIDLE :: 4;
TCP_KEEPINTVL :: 5;
TCP_LINGER2 :: 8;
TCP_MAXSEG :: 2;
TCP_MAXWIN :: 65535;
TCP_MAX_WINSHIFT :: 14;
TCP_MD5SIG :: 14;
TCP_MD5SIG_EXT :: 32;
TCP_MD5SIG_FLAG_PREFIX :: 1;
TCP_MD5SIG_MAXKEYLEN :: 80;
TCP_MSS :: 512;
TCP_MSS_DEFAULT :: 536;
TCP_MSS_DESIRED :: 1220;
TCP_NODELAY :: 1;
TCP_NOTSENT_LOWAT :: 25;
TCP_QUEUE_SEQ :: 21;
TCP_QUICKACK :: 12;
TCP_REPAIR :: 19;
TCP_REPAIR_OPTIONS :: 22;
TCP_REPAIR_QUEUE :: 20;
TCP_REPAIR_WINDOW :: 29;
TCP_SAVED_SYN :: 28;
TCP_SAVE_SYN :: 27;
TCP_SYNCNT :: 7;
TCP_S_DATA_IN :: 4;
TCP_S_DATA_OUT :: 8;
TCP_THIN_DUPACK :: 17;
TCP_THIN_LINEAR_TIMEOUTS :: 16;
TCP_TIMESTAMP :: 24;
TCP_ULP :: 31;
TCP_USER_TIMEOUT :: 18;
TCP_WINDOW_CLAMP :: 10;
TH_ACK :: 16;
TH_FIN :: 1;
TH_PUSH :: 8;
TH_RST :: 4;
TH_SYN :: 2;
TH_URG :: 32;
TRY_AGAIN :: 2;
_PATH_HEQUIV :: 93824992253708;
_PATH_HOSTS :: 93824992253737;
_PATH_NETWORKS :: 93824992253763;
_PATH_NSSWITCH_CONF :: 93824992253797;
_PATH_PROTOCOLS :: 93824992253832;
_PATH_SERVICES :: 93824992253862;
__USE_KERNEL_IPV6_DEFS :: 0;

// ------------------------------------------------------------- the types ---

__fd_mask :: s64;
fd_set :: struct {
    fds_bits: [16] __fd_mask;
}

fd_mask :: __fd_mask;
socklen_t :: __socklen_t;
sa_family_t :: u16;
sockaddr :: struct {
    sa_family: sa_family_t;
    sa_data: [14] u8;
}

sockaddr_storage :: struct {
    ss_family: sa_family_t;
    __ss_padding: [118] u8;
    __ss_align: u64;
}

msghdr :: struct {
    msg_name: *void;
    msg_namelen: socklen_t;
    msg_iov: *iovec;
    msg_iovlen: size_t;
    msg_control: *void;
    msg_controllen: size_t;
    msg_flags: s32;
}

cmsghdr :: struct {
    cmsg_len: size_t;
    cmsg_level: s32;
    cmsg_type: s32;
    __cmsg_data: [1] u8;
}

ucred :: struct {
    pid: pid_t;
    uid: uid_t;
    gid: gid_t;
}

linger :: struct {
    l_onoff: s32;
    l_linger: s32;
}

mmsghdr :: struct {
    msg_hdr: msghdr;
    msg_len: u32;
}

in_addr_t :: uint32_t;
in_addr :: struct {
    s_addr: in_addr_t;
}

ip_opts :: struct {
    ip_dst: in_addr;
    ip_opts: [40] u8;
}

in_pktinfo :: struct {
    ipi_ifindex: s32;
    ipi_spec_dst: in_addr;
    ipi_addr: in_addr;
}

in_port_t :: uint16_t;
in6_addr :: struct {
    __in6_u: union {
        __u6_addr8: [16] uint8_t;
        __u6_addr16: [8] uint16_t;
        __u6_addr32: [4] uint32_t;
    }
}

sockaddr_in :: struct {
    sin_family: sa_family_t;
    sin_port: in_port_t;
    sin_addr: in_addr;
    sin_zero: [8] u8;
}

sockaddr_in6 :: struct {
    sin6_family: sa_family_t;
    sin6_port: in_port_t;
    sin6_flowinfo: uint32_t;
    sin6_addr: in6_addr;
    sin6_scope_id: uint32_t;
}

ip_mreq :: struct {
    imr_multiaddr: in_addr;
    imr_interface: in_addr;
}

ip_mreqn :: struct {
    imr_multiaddr: in_addr;
    imr_address: in_addr;
    imr_ifindex: s32;
}

ip_mreq_source :: struct {
    imr_multiaddr: in_addr;
    imr_interface: in_addr;
    imr_sourceaddr: in_addr;
}

ipv6_mreq :: struct {
    ipv6mr_multiaddr: in6_addr;
    ipv6mr_interface: u32;
}

group_req :: struct {
    gr_interface: uint32_t;
    gr_group: sockaddr_storage;
}

group_source_req :: struct {
    gsr_interface: uint32_t;
    gsr_group: sockaddr_storage;
    gsr_source: sockaddr_storage;
}

ip_msfilter :: struct {
    imsf_multiaddr: in_addr;
    imsf_interface: in_addr;
    imsf_fmode: uint32_t;
    imsf_numsrc: uint32_t;
    imsf_slist: [1] in_addr;
}

group_filter :: struct {
    gf_interface: uint32_t;
    gf_group: sockaddr_storage;
    gf_fmode: uint32_t;
    gf_numsrc: uint32_t;
    gf_slist: [1] sockaddr_storage;
}

in6_pktinfo :: struct {
    ipi6_addr: in6_addr;
    ipi6_ifindex: u32;
}

ip6_mtuinfo :: struct {
    ip6m_addr: sockaddr_in6;
    ip6m_mtu: uint32_t;
}

tcp_seq :: uint32_t;
tcphdr :: struct {
    using _anonymous_0: union {
        using _anonymous_0: struct {
            th_sport: uint16_t;
            th_dport: uint16_t;
            th_seq: tcp_seq;
            th_ack: tcp_seq;
            // bitfield dropped: uint8_t th_x2 : 4
            // bitfield dropped: uint8_t th_off : 4
            th_flags: uint8_t;
            th_win: uint16_t;
            th_sum: uint16_t;
            th_urp: uint16_t;
        }
        using _anonymous_12: struct {
            source: uint16_t;
            dest: uint16_t;
            seq: uint32_t;
            ack_seq: uint32_t;
            // bitfield dropped: uint16_t res1 : 4
            // bitfield dropped: uint16_t doff : 4
            // bitfield dropped: uint16_t fin : 1
            // bitfield dropped: uint16_t syn : 1
            // bitfield dropped: uint16_t rst : 1
            // bitfield dropped: uint16_t psh : 1
            // bitfield dropped: uint16_t ack : 1
            // bitfield dropped: uint16_t urg : 1
            // bitfield dropped: uint16_t res2 : 2
            window: uint16_t;
            check: uint16_t;
            urg_ptr: uint16_t;
        }
    }
}

tcp_info :: struct {
    tcpi_state: uint8_t;
    tcpi_ca_state: uint8_t;
    tcpi_retransmits: uint8_t;
    tcpi_probes: uint8_t;
    tcpi_backoff: uint8_t;
    tcpi_options: uint8_t;
    // bitfield dropped: uint8_t tcpi_snd_wscale : 4
    // bitfield dropped: uint8_t tcpi_rcv_wscale : 4
    tcpi_rto: uint32_t;
    tcpi_ato: uint32_t;
    tcpi_snd_mss: uint32_t;
    tcpi_rcv_mss: uint32_t;
    tcpi_unacked: uint32_t;
    tcpi_sacked: uint32_t;
    tcpi_lost: uint32_t;
    tcpi_retrans: uint32_t;
    tcpi_fackets: uint32_t;
    tcpi_last_data_sent: uint32_t;
    tcpi_last_ack_sent: uint32_t;
    tcpi_last_data_recv: uint32_t;
    tcpi_last_ack_recv: uint32_t;
    tcpi_pmtu: uint32_t;
    tcpi_rcv_ssthresh: uint32_t;
    tcpi_rtt: uint32_t;
    tcpi_rttvar: uint32_t;
    tcpi_snd_ssthresh: uint32_t;
    tcpi_snd_cwnd: uint32_t;
    tcpi_advmss: uint32_t;
    tcpi_reordering: uint32_t;
    tcpi_rcv_rtt: uint32_t;
    tcpi_rcv_space: uint32_t;
    tcpi_total_retrans: uint32_t;
}

tcp_md5sig :: struct {
    tcpm_addr: sockaddr_storage;
    tcpm_flags: uint8_t;
    tcpm_prefixlen: uint8_t;
    tcpm_keylen: uint16_t;
    tcpm_ifindex: s32;
    tcpm_key: [80] uint8_t;
}

tcp_repair_opt :: struct {
    opt_code: uint32_t;
    opt_val: uint32_t;
}

tcp_cookie_transactions :: struct {
    tcpct_flags: uint16_t;
    __tcpct_pad1: uint8_t;
    tcpct_cookie_desired: uint8_t;
    tcpct_s_data_desired: uint16_t;
    tcpct_used: uint16_t;
    tcpct_value: [536] uint8_t;
}

tcp_repair_window :: struct {
    snd_wl1: uint32_t;
    snd_wnd: uint32_t;
    max_window: uint32_t;
    rcv_wnd: uint32_t;
    rcv_wup: uint32_t;
}

netent :: struct {
    n_name: *u8;
    n_aliases: **u8;
    n_addrtype: s32;
    n_net: uint32_t;
}

hostent :: struct {
    h_name: *u8;
    h_aliases: **u8;
    h_addrtype: s32;
    h_length: s32;
    h_addr_list: **u8;
}

servent :: struct {
    s_name: *u8;
    s_aliases: **u8;
    s_port: s32;
    s_proto: *u8;
}

protoent :: struct {
    p_name: *u8;
    p_aliases: **u8;
    p_proto: s32;
}

AI :: enum_flags u32 {
    PASSIVE                  :: 0x0001;
    CANONNAME                :: 0x0002;
    NUMERICHOST              :: 0x0004;
    V4MAPPED                 :: 0x0008;
    ALL                      :: 0x0010;
    ADDRCONFIG               :: 0x0020;
    IDN                      :: 0x0040;
    CANONIDN                 :: 0x0080;
    IDN_ALLOW_UNASSIGNED     :: 0x0100;
    IDN_USE_STD3_ASCII_RULES :: 0x0200;
    NUMERICSERV              :: 0x0400;

    AI_PASSIVE                  :: PASSIVE;
    AI_CANONNAME                :: CANONNAME;
    AI_NUMERICHOST              :: NUMERICHOST;
    AI_V4MAPPED                 :: V4MAPPED;
    AI_ALL                      :: ALL;
    AI_ADDRCONFIG               :: ADDRCONFIG;
    AI_IDN                      :: IDN;
    AI_CANONIDN                 :: CANONIDN;
    AI_IDN_ALLOW_UNASSIGNED     :: IDN_ALLOW_UNASSIGNED;
    AI_IDN_USE_STD3_ASCII_RULES :: IDN_USE_STD3_ASCII_RULES;
    AI_NUMERICSERV              :: NUMERICSERV;
}

addrinfo :: struct {
    ai_flags: AI;
    ai_family: s32;
    ai_socktype: SOCK;
    ai_protocol: IPPROTO;
    ai_addrlen: socklen_t;
    ai_addr: *sockaddr;
    ai_canonname: *u8;
    ai_next: *addrinfo;
}

gaicb :: struct {
    ar_name: *u8;
    ar_service: *u8;
    ar_request: *addrinfo;
    ar_result: *addrinfo;
    __return: s32;
    __glibc_reserved: [5] s32;
}

if_nameindex :: struct {
    if_index: u32;
    if_name: *u8;
}

ifaddr :: struct {
    ifa_addr: sockaddr;
    ifa_ifu: union {
        ifu_broadaddr: sockaddr;
        ifu_dstaddr: sockaddr;
    }
    ifa_ifp: *iface;
    ifa_next: *ifaddr;
}

ifmap :: struct {
    mem_start: u64;
    mem_end: u64;
    base_addr: u16;
    irq: u8;
    dma: u8;
    port: u8;
}

ifreq :: struct {
    ifr_ifrn: union {
        ifrn_name: [16] u8;
    }
    ifr_ifru: union {
        ifru_addr: sockaddr;
        ifru_dstaddr: sockaddr;
        ifru_broadaddr: sockaddr;
        ifru_netmask: sockaddr;
        ifru_hwaddr: sockaddr;
        ifru_flags: s16;
        ifru_ivalue: s32;
        ifru_mtu: s32;
        ifru_map: ifmap;
        ifru_slave: [16] u8;
        ifru_newname: [16] u8;
        ifru_data: __caddr_t;
    }
}

ifconf :: struct {
    ifc_len: s32;
    ifc_ifcu: union {
        ifcu_buf: __caddr_t;
        ifcu_req: *ifreq;
    }
}

ifaddrs :: struct {
    ifa_next: *ifaddrs;
    ifa_name: *u8;
    ifa_flags: u32;
    ifa_addr: *sockaddr;
    ifa_netmask: *sockaddr;
    ifa_ifu: union {
        ifu_broadaddr: *sockaddr;
        ifu_dstaddr: *sockaddr;
    }
    ifa_data: *void;
}

__caddr_t :: *u8;
__socklen_t :: u32;
__SOCKADDR_ARG :: union {
    __sockaddr__: *sockaddr;
    __sockaddr_at__: *sockaddr_at;
    __sockaddr_ax25__: *sockaddr_ax25;
    __sockaddr_dl__: *sockaddr_dl;
    __sockaddr_eon__: *sockaddr_eon;
    __sockaddr_in__: *sockaddr_in;
    __sockaddr_in6__: *sockaddr_in6;
    __sockaddr_inarp__: *sockaddr_inarp;
    __sockaddr_ipx__: *sockaddr_ipx;
    __sockaddr_iso__: *sockaddr_iso;
    __sockaddr_ns__: *sockaddr_ns;
    __sockaddr_un__: *sockaddr_un;
    __sockaddr_x25__: *sockaddr_x25;
}

__CONST_SOCKADDR_ARG :: union {
    __sockaddr__: *sockaddr;
    __sockaddr_at__: *sockaddr_at;
    __sockaddr_ax25__: *sockaddr_ax25;
    __sockaddr_dl__: *sockaddr_dl;
    __sockaddr_eon__: *sockaddr_eon;
    __sockaddr_in__: *sockaddr_in;
    __sockaddr_in6__: *sockaddr_in6;
    __sockaddr_inarp__: *sockaddr_inarp;
    __sockaddr_ipx__: *sockaddr_ipx;
    __sockaddr_iso__: *sockaddr_iso;
    __sockaddr_ns__: *sockaddr_ns;
    __sockaddr_un__: *sockaddr_un;
    __sockaddr_x25__: *sockaddr_x25;
}

sockaddr_un :: struct {
    sun_family: sa_family_t;
    sun_path: [108] u8;
}

// Opaque: the headers name it only behind a pointer.
iface :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_at :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_ax25 :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_dl :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_eon :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_inarp :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_ipx :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_iso :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_ns :: struct {}

// Opaque: the headers name it only behind a pointer.
sockaddr_x25 :: struct {}


// ------------------------------------------------------------------ enums ---

SOCK :: enum s32 {
    SOCK_STREAM :: 1;
    SOCK_DGRAM :: 2;
    SOCK_RAW :: 3;
    SOCK_RDM :: 4;
    SOCK_SEQPACKET :: 5;
    SOCK_DCCP :: 6;
    SOCK_PACKET :: 10;
    SOCK_CLOEXEC :: 524288;
    SOCK_NONBLOCK :: 2048;
}
__socket_type :: SOCK;

MSG :: enum s32 {
    OOB :: 1;
    PEEK :: 2;
    DONTROUTE :: 4;
    TRYHARD :: DONTROUTE;
    CTRUNC :: 8;
    PROXY :: 16;
    TRUNC :: 32;
    DONTWAIT :: 64;
    EOR :: 128;
    WAITALL :: 256;
    FIN :: 512;
    SYN :: 1024;
    CONFIRM :: 2048;
    RST :: 4096;
    ERRQUEUE :: 8192;
    NOSIGNAL :: 16384;
    MORE :: 32768;
    WAITFORONE :: 65536;
    BATCH :: 262144;
    SOCK_DEVMEM :: 33554432;
    ZEROCOPY :: 67108864;
    FASTOPEN :: 536870912;
    CMSG_CLOEXEC :: 1073741824;
}

SCM :: enum s32 {
    RIGHTS :: 1;
    CREDENTIALS :: 2;
    SECURITY :: 3;
    PIDFD :: 4;
}

SHUT :: enum s32 {
    RD :: 0;
    WR;
    RDWR;
}

IPPROTO :: enum s32 {
    IP :: 0;
    ICMP :: 1;
    IGMP :: 2;
    IPIP :: 4;
    TCP :: 6;
    EGP :: 8;
    PUP :: 12;
    UDP :: 17;
    IDP :: 22;
    TP :: 29;
    DCCP :: 33;
    IPV6 :: 41;
    RSVP :: 46;
    GRE :: 47;
    ESP :: 50;
    AH :: 51;
    MTP :: 92;
    BEETPH :: 94;
    ENCAP :: 98;
    PIM :: 103;
    COMP :: 108;
    L2TP :: 115;
    SCTP :: 132;
    UDPLITE :: 136;
    MPLS :: 137;
    ETHERNET :: 143;
    RAW :: 255;
    SMC :: 256;
    MPTCP :: 262;
    MAX;
}

IPPROTO_1 :: enum s32 {
    HOPOPTS :: 0;
    ROUTING :: 43;
    FRAGMENT :: 44;
    ICMPV6 :: 58;
    NONE :: 59;
    DSTOPTS :: 60;
    MH :: 135;
}

IPPORT :: enum s32 {
    ECHO :: 7;
    DISCARD :: 9;
    SYSTAT :: 11;
    DAYTIME :: 13;
    NETSTAT :: 15;
    FTP :: 21;
    TELNET :: 23;
    SMTP :: 25;
    TIMESERVER :: 37;
    NAMESERVER :: 42;
    WHOIS :: 43;
    MTP :: 57;
    TFTP :: 69;
    RJE :: 77;
    FINGER :: 79;
    TTYLINK :: 87;
    SUPDUP :: 95;
    EXECSERVER :: 512;
    LOGINSERVER :: 513;
    CMDSERVER :: 514;
    EFSSERVER :: 520;
    BIFFUDP :: 512;
    WHOSERVER :: 513;
    ROUTESERVER :: 520;
    RESERVED :: 1024;
    USERRESERVED :: 5000;
}

TCP :: enum s32 {
    ESTABLISHED :: 1;
    SYN_SENT;
    SYN_RECV;
    FIN_WAIT1;
    FIN_WAIT2;
    TIME_WAIT;
    CLOSE;
    CLOSE_WAIT;
    LAST_ACK;
    LISTEN;
    CLOSING;
}

tcp_ca_state :: enum s32 {
    TCP_CA_Open :: 0;
    TCP_CA_Disorder :: 1;
    TCP_CA_CWR :: 2;
    TCP_CA_Recovery :: 3;
    TCP_CA_Loss :: 4;
}

TCP_1 :: enum s32 {
    NO_QUEUE;
    RECV_QUEUE;
    SEND_QUEUE;
    QUEUES_NR;
}

IFF :: enum s32 {
    UP :: 1;
    BROADCAST :: 2;
    DEBUG :: 4;
    LOOPBACK :: 8;
    POINTOPOINT :: 16;
    NOTRAILERS :: 32;
    RUNNING :: 64;
    NOARP :: 128;
    PROMISC :: 256;
    ALLMULTI :: 512;
    MASTER :: 1024;
    SLAVE :: 2048;
    MULTICAST :: 4096;
    PORTSEL :: 8192;
    AUTOMEDIA :: 16384;
    DYNAMIC :: 32768;
}


// ------------------------------------------------------------- procedures ---

__cmsg_nxthdr :: (__mhdr: *msghdr, __cmsg: *cmsghdr) -> *cmsghdr #foreign libc;
__h_errno_location :: () -> *s32 #foreign libc;
accept :: (__fd: s32, __addr: __SOCKADDR_ARG, __addr_len: *socklen_t) -> s32 #foreign libc;
accept4 :: (__fd: s32, __addr: __SOCKADDR_ARG, __addr_len: *socklen_t, __flags: s32) -> s32 #foreign libc;
bind :: (__fd: s32, __addr: __CONST_SOCKADDR_ARG, __len: socklen_t) -> s32 #foreign libc;
bindresvport :: (__sockfd: s32, __sock_in: *sockaddr_in) -> s32 #foreign libc;
connect :: (__fd: s32, __addr: __CONST_SOCKADDR_ARG, __len: socklen_t) -> s32 #foreign libc;
endhostent :: () -> void #foreign libc;
endnetent :: () -> void #foreign libc;
endnetgrent :: () -> void #foreign libc;
endprotoent :: () -> void #foreign libc;
endservent :: () -> void #foreign libc;
freeaddrinfo :: (__ai: *addrinfo) -> void #foreign libc;
freeifaddrs :: (__ifa: *ifaddrs) -> void #foreign libc;
gai_strerror :: (__ecode: s32) -> *u8 #foreign libc;
getaddrinfo :: (__name: *u8, __service: *u8, __req: *addrinfo, __pai: **addrinfo) -> s32 #foreign libc;
gethostbyaddr :: (__addr: *void, __len: __socklen_t, __type: s32) -> *hostent #foreign libc;
gethostbyaddr_r :: (__addr: *void, __len: __socklen_t, __type: s32, __result_buf: *hostent, __buf: *u8, __buflen: size_t, __result: **hostent, __h_errnop: *s32) -> s32 #foreign libc;
gethostbyname :: (__name: *u8) -> *hostent #foreign libc;
gethostbyname2 :: (__name: *u8, __af: s32) -> *hostent #foreign libc;
gethostbyname2_r :: (__name: *u8, __af: s32, __result_buf: *hostent, __buf: *u8, __buflen: size_t, __result: **hostent, __h_errnop: *s32) -> s32 #foreign libc;
gethostbyname_r :: (__name: *u8, __result_buf: *hostent, __buf: *u8, __buflen: size_t, __result: **hostent, __h_errnop: *s32) -> s32 #foreign libc;
gethostent :: () -> *hostent #foreign libc;
gethostent_r :: (__result_buf: *hostent, __buf: *u8, __buflen: size_t, __result: **hostent, __h_errnop: *s32) -> s32 #foreign libc;
getifaddrs :: (__ifap: **ifaddrs) -> s32 #foreign libc;
getipv4sourcefilter :: (__s: s32, __interface_addr: in_addr, __group: in_addr, __fmode: *uint32_t, __numsrc: *uint32_t, __slist: *in_addr) -> s32 #foreign libc;
getnameinfo :: (__sa: *sockaddr, __salen: socklen_t, __host: *u8, __hostlen: socklen_t, __serv: *u8, __servlen: socklen_t, __flags: s32) -> s32 #foreign libc;
getnetbyaddr :: (__net: uint32_t, __type: s32) -> *netent #foreign libc;
getnetbyaddr_r :: (__net: uint32_t, __type: s32, __result_buf: *netent, __buf: *u8, __buflen: size_t, __result: **netent, __h_errnop: *s32) -> s32 #foreign libc;
getnetbyname :: (__name: *u8) -> *netent #foreign libc;
getnetbyname_r :: (__name: *u8, __result_buf: *netent, __buf: *u8, __buflen: size_t, __result: **netent, __h_errnop: *s32) -> s32 #foreign libc;
getnetent :: () -> *netent #foreign libc;
getnetent_r :: (__result_buf: *netent, __buf: *u8, __buflen: size_t, __result: **netent, __h_errnop: *s32) -> s32 #foreign libc;
getnetgrent :: (__hostp: **u8, __userp: **u8, __domainp: **u8) -> s32 #foreign libc;
getnetgrent_r :: (__hostp: **u8, __userp: **u8, __domainp: **u8, __buffer: *u8, __buflen: size_t) -> s32 #foreign libc;
getpeername :: (__fd: s32, __addr: __SOCKADDR_ARG, __len: *socklen_t) -> s32 #foreign libc;
getprotobyname :: (__name: *u8) -> *protoent #foreign libc;
getprotobyname_r :: (__name: *u8, __result_buf: *protoent, __buf: *u8, __buflen: size_t, __result: **protoent) -> s32 #foreign libc;
getprotobynumber :: (__proto: s32) -> *protoent #foreign libc;
getprotobynumber_r :: (__proto: s32, __result_buf: *protoent, __buf: *u8, __buflen: size_t, __result: **protoent) -> s32 #foreign libc;
getprotoent :: () -> *protoent #foreign libc;
getprotoent_r :: (__result_buf: *protoent, __buf: *u8, __buflen: size_t, __result: **protoent) -> s32 #foreign libc;
getservbyname :: (__name: *u8, __proto: *u8) -> *servent #foreign libc;
getservbyname_r :: (__name: *u8, __proto: *u8, __result_buf: *servent, __buf: *u8, __buflen: size_t, __result: **servent) -> s32 #foreign libc;
getservbyport :: (__port: s32, __proto: *u8) -> *servent #foreign libc;
getservbyport_r :: (__port: s32, __proto: *u8, __result_buf: *servent, __buf: *u8, __buflen: size_t, __result: **servent) -> s32 #foreign libc;
getservent :: () -> *servent #foreign libc;
getservent_r :: (__result_buf: *servent, __buf: *u8, __buflen: size_t, __result: **servent) -> s32 #foreign libc;
getsockname :: (__fd: s32, __addr: __SOCKADDR_ARG, __len: *socklen_t) -> s32 #foreign libc;
getsockopt :: (__fd: s32, __level: s32, __optname: s32, __optval: *void, __optlen: *socklen_t) -> s32 #foreign libc;
getsourcefilter :: (__s: s32, __interface_addr: uint32_t, __group: *sockaddr, __grouplen: socklen_t, __fmode: *uint32_t, __numsrc: *uint32_t, __slist: *sockaddr_storage) -> s32 #foreign libc;
herror :: (__str: *u8) -> void #foreign libc;
hstrerror :: (__err_num: s32) -> *u8 #foreign libc;
htonl :: (__hostlong: uint32_t) -> uint32_t #foreign libc;
htons :: (__hostshort: uint16_t) -> uint16_t #foreign libc;
if_freenameindex :: (__ptr: *if_nameindex) -> void #foreign libc;
if_indextoname :: (__ifindex: u32, __ifname: *u8) -> *u8 #foreign libc;
if_nametoindex :: (__ifname: *u8) -> u32 #foreign libc;
inet6_opt_append :: (__extbuf: *void, __extlen: socklen_t, __offset: s32, __type: uint8_t, __len: socklen_t, __align: uint8_t, __databufp: **void) -> s32 #foreign libc;
inet6_opt_find :: (__extbuf: *void, __extlen: socklen_t, __offset: s32, __type: uint8_t, __lenp: *socklen_t, __databufp: **void) -> s32 #foreign libc;
inet6_opt_finish :: (__extbuf: *void, __extlen: socklen_t, __offset: s32) -> s32 #foreign libc;
inet6_opt_get_val :: (__databuf: *void, __offset: s32, __val: *void, __vallen: socklen_t) -> s32 #foreign libc;
inet6_opt_init :: (__extbuf: *void, __extlen: socklen_t) -> s32 #foreign libc;
inet6_opt_next :: (__extbuf: *void, __extlen: socklen_t, __offset: s32, __typep: *uint8_t, __lenp: *socklen_t, __databufp: **void) -> s32 #foreign libc;
inet6_opt_set_val :: (__databuf: *void, __offset: s32, __val: *void, __vallen: socklen_t) -> s32 #foreign libc;
inet6_option_alloc :: (__cmsg: *cmsghdr, __datalen: s32, __multx: s32, __plusy: s32) -> *uint8_t #foreign libc;
inet6_option_append :: (__cmsg: *cmsghdr, __typep: *uint8_t, __multx: s32, __plusy: s32) -> s32 #foreign libc;
inet6_option_find :: (__cmsg: *cmsghdr, __tptrp: **uint8_t, __type: s32) -> s32 #foreign libc;
inet6_option_init :: (__bp: *void, __cmsgp: **cmsghdr, __type: s32) -> s32 #foreign libc;
inet6_option_next :: (__cmsg: *cmsghdr, __tptrp: **uint8_t) -> s32 #foreign libc;
inet6_option_space :: (__nbytes: s32) -> s32 #foreign libc;
inet6_rth_add :: (__bp: *void, __addr: *in6_addr) -> s32 #foreign libc;
inet6_rth_getaddr :: (__bp: *void, __index: s32) -> *in6_addr #foreign libc;
inet6_rth_init :: (__bp: *void, __bp_len: socklen_t, __type: s32, __segments: s32) -> *void #foreign libc;
inet6_rth_reverse :: (__in: *void, __out: *void) -> s32 #foreign libc;
inet6_rth_segments :: (__bp: *void) -> s32 #foreign libc;
inet6_rth_space :: (__type: s32, __segments: s32) -> socklen_t #foreign libc;
inet_addr :: (__cp: *u8) -> in_addr_t #foreign libc;
inet_aton :: (__cp: *u8, __inp: *in_addr) -> s32 #foreign libc;
inet_lnaof :: (__in: in_addr) -> in_addr_t #foreign libc;
inet_makeaddr :: (__net: in_addr_t, __host: in_addr_t) -> in_addr #foreign libc;
inet_netof :: (__in: in_addr) -> in_addr_t #foreign libc;
inet_network :: (__cp: *u8) -> in_addr_t #foreign libc;
inet_nsap_addr :: (__cp: *u8, __buf: *u8, __len: s32) -> u32 #foreign libc;
inet_nsap_ntoa :: (__len: s32, __cp: *u8, __buf: *u8) -> *u8 #foreign libc;
inet_ntoa :: (__in: in_addr) -> *u8 #foreign libc;
inet_ntop :: (__af: s32, __cp: *void, __buf: *u8, __len: socklen_t) -> *u8 #foreign libc;
inet_pton :: (__af: s32, __cp: *u8, __buf: *void) -> s32 #foreign libc;
innetgr :: (__netgroup: *u8, __host: *u8, __user: *u8, __domain: *u8) -> s32 #foreign libc;
iruserok :: (__raddr: uint32_t, __suser: s32, __remuser: *u8, __locuser: *u8) -> s32 #foreign libc;
iruserok_af :: (__raddr: *void, __suser: s32, __remuser: *u8, __locuser: *u8, __af: sa_family_t) -> s32 #foreign libc;
isfdtype :: (__fd: s32, __fdtype: s32) -> s32 #foreign libc;
listen :: (__fd: s32, __n: s32) -> s32 #foreign libc;
ntohl :: (__netlong: uint32_t) -> uint32_t #foreign libc;
ntohs :: (__netshort: uint16_t) -> uint16_t #foreign libc;
pselect :: (__nfds: s32, __readfds: *fd_set, __writefds: *fd_set, __exceptfds: *fd_set, __timeout: *timespec, __sigmask: *__sigset_t) -> s32 #foreign libc;
rcmd :: (__ahost: **u8, __rport: u16, __locuser: *u8, __remuser: *u8, __cmd: *u8, __fd2p: *s32) -> s32 #foreign libc;
rcmd_af :: (__ahost: **u8, __rport: u16, __locuser: *u8, __remuser: *u8, __cmd: *u8, __fd2p: *s32, __af: sa_family_t) -> s32 #foreign libc;
recv :: (__fd: s32, __buf: *void, __n: size_t, __flags: s32) -> ssize_t #foreign libc;
recvfrom :: (__fd: s32, __buf: *void, __n: size_t, __flags: s32, __addr: __SOCKADDR_ARG, __addr_len: *socklen_t) -> ssize_t #foreign libc;
recvmmsg :: (__fd: s32, __vmessages: *mmsghdr, __vlen: u32, __flags: s32, __tmo: *timespec) -> s32 #foreign libc;
recvmsg :: (__fd: s32, __message: *msghdr, __flags: s32) -> ssize_t #foreign libc;
rexec :: (__ahost: **u8, __rport: s32, __name: *u8, __pass: *u8, __cmd: *u8, __fd2p: *s32) -> s32 #foreign libc;
rexec_af :: (__ahost: **u8, __rport: s32, __name: *u8, __pass: *u8, __cmd: *u8, __fd2p: *s32, __af: sa_family_t) -> s32 #foreign libc;
rresvport :: (__alport: *s32) -> s32 #foreign libc;
rresvport_af :: (__alport: *s32, __af: sa_family_t) -> s32 #foreign libc;
ruserok :: (__rhost: *u8, __suser: s32, __remuser: *u8, __locuser: *u8) -> s32 #foreign libc;
ruserok_af :: (__rhost: *u8, __suser: s32, __remuser: *u8, __locuser: *u8, __af: sa_family_t) -> s32 #foreign libc;
select :: (__nfds: s32, __readfds: *fd_set, __writefds: *fd_set, __exceptfds: *fd_set, __timeout: *timeval) -> s32 #foreign libc;
send :: (__fd: s32, __buf: *void, __n: size_t, __flags: s32) -> ssize_t #foreign libc;
sendmmsg :: (__fd: s32, __vmessages: *mmsghdr, __vlen: u32, __flags: s32) -> s32 #foreign libc;
sendmsg :: (__fd: s32, __message: *msghdr, __flags: s32) -> ssize_t #foreign libc;
sendto :: (__fd: s32, __buf: *void, __n: size_t, __flags: s32, __addr: __CONST_SOCKADDR_ARG, __addr_len: socklen_t) -> ssize_t #foreign libc;
sethostent :: (__stay_open: s32) -> void #foreign libc;
setipv4sourcefilter :: (__s: s32, __interface_addr: in_addr, __group: in_addr, __fmode: uint32_t, __numsrc: uint32_t, __slist: *in_addr) -> s32 #foreign libc;
setnetent :: (__stay_open: s32) -> void #foreign libc;
setnetgrent :: (__netgroup: *u8) -> s32 #foreign libc;
setprotoent :: (__stay_open: s32) -> void #foreign libc;
setservent :: (__stay_open: s32) -> void #foreign libc;
setsockopt :: (__fd: s32, __level: s32, __optname: s32, __optval: *void, __optlen: socklen_t) -> s32 #foreign libc;
setsourcefilter :: (__s: s32, __interface_addr: uint32_t, __group: *sockaddr, __grouplen: socklen_t, __fmode: uint32_t, __numsrc: uint32_t, __slist: *sockaddr_storage) -> s32 #foreign libc;
shutdown :: (__fd: s32, __how: s32) -> s32 #foreign libc;
sockatmark :: (__fd: s32) -> s32 #foreign libc;
socket :: (__domain: s32, __type: s32, __protocol: s32) -> s32 #foreign libc;
socketpair :: (__domain: s32, __type: s32, __protocol: s32, __fds: *s32) -> s32 #foreign libc;

// ---------------------------------------------------------- hand-written ---
//
// What no generator can read out of a header: C's macros, and the handful of
// conveniences the reference builds on top of the raw calls.

// On Linux a socket is a file descriptor; Windows has a handle type of its
// own, which is why this is a name rather than `s32` everywhere it is used.
Socket :: s32;

INVALID_SOCKET     :: cast,trunc(Socket) (~0);
SOCKET_ERROR       :: -1;
SOCKET_WOULDBLOCK  :: EWOULDBLOCK;

// ------------------------------------------------------------- fd_set ---
//
// `select` takes a bitmap of descriptors. C does the bit arithmetic in macros;
// these are the same arithmetic, with the bounds checked so that a descriptor
// past the end of the set is ignored rather than corrupting memory.

FD_BITS_ELEMENT_TYPE :: type_of(fd_set.fds_bits[0]);
BITS_PER_FD_SET_MASK :: size_of(FD_BITS_ELEMENT_TYPE) * 8;

check_fd_set :: (fd: s32, set: *fd_set) -> bool {
    if !set  return false;
    return fd >= 0 && fd / BITS_PER_FD_SET_MASK < set.fds_bits.count;
}

FD_SET :: (fd: s32, set: *fd_set) {
    if !check_fd_set(fd, set)  return;
    set.fds_bits[fd / BITS_PER_FD_SET_MASK] |= cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
}

FD_CLR :: (fd: s32, set: *fd_set) {
    if !check_fd_set(fd, set)  return;
    set.fds_bits[fd / BITS_PER_FD_SET_MASK] &= ~cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
}

FD_ISSET :: (fd: s32, set: *fd_set) -> bool {
    if !check_fd_set(fd, set)  return false;
    mask := cast,no_check(FD_BITS_ELEMENT_TYPE) (cast(u64) 1 << (cast(u64) fd % BITS_PER_FD_SET_MASK));
    return (set.fds_bits[fd / BITS_PER_FD_SET_MASK] & mask) != 0;
}

// ----------------------------------------------------------- ancillary ---
//
// Control messages travel in a buffer as a chain of headers, each padded out
// to a pointer boundary. These walk it.

CMSG_ALIGN :: (length: u64) -> u64 {
    return (length + size_of(u64) - 1) & ~cast(u64) (size_of(u64) - 1);
}

// How much buffer a payload of `length` needs, header and padding included.
CMSG_SIZE :: (length: u64) -> u64 {
    return CMSG_ALIGN(cast(u64) size_of(cmsghdr)) + length;
}

// What goes in `cmsg_len`: the header is counted unpadded, since the padding
// belongs to whatever follows.
CMSG_LEN :: (length: u64) -> u64 {
    return CMSG_ALIGN(cast(u64) size_of(cmsghdr)) + length;
}

// The payload just past a header.
CMSG_DATA :: (header: *cmsghdr) -> *u8 {
    return cast(*u8) header + CMSG_ALIGN(cast(u64) size_of(cmsghdr));
}

// The first header in a message's control buffer, or null when there is none.
CMSG_FIRSTHDR :: (message: *msghdr) -> *cmsghdr {
    if !message || message.msg_controllen < cast(u64) size_of(cmsghdr)  return null;
    return cast(*cmsghdr) message.msg_control;
}

// The well-known addresses, which C declares as globals.
in6addr_any      : in6_addr #elsewhere libc;
in6addr_loopback : in6_addr #elsewhere libc;

// `if_nameindex` is a struct and a function in C, and Jai has one namespace
// for both. The struct keeps the name, so the call gets the trailing
// underscore the reference gives it.
if_nameindex_ :: () -> *if_nameindex #foreign libc "if_nameindex";

// ------------------------------------------------------- the conveniences ---
//
// What the reference builds on top of the raw calls. On Linux most of these
// have nothing platform-specific to do; they exist so that a program written
// against the module runs on Windows too, where they do.

// Winsock needs starting; Unix does not.
socket_init :: () -> bool {
    return true;
}

get_last_socket_error :: () -> OS_Error_Code {
    return cast(OS_Error_Code) errno();
}

// Turns the descriptor's non-blocking flag on or off, leaving the rest alone.
set_blocking :: (s: Socket, blocking: bool) -> bool {
    value := fcntl(s, F_GETFL, 0);
    if value < 0  return false;

    if blocking  value &= ~O_NONBLOCK;
    else         value |= O_NONBLOCK;
    return fcntl(s, F_SETFL, value) == 0;
}

// Keep-alive probes on a connection, and how patient they are. A zero leaves
// that particular timer at the system default.
set_keepalive :: (s: Socket, keepalive: bool, idle_seconds_before_keepalive: s32 = 0, keepalive_interval_seconds: s32 = 0, num_keepalive_attempts: s32 = 0) -> bool {
    on: s32 = ifx keepalive then 1 else 0;
    if setsockopt(s, SOL_SOCKET, SO_KEEPALIVE, *on, size_of(s32)) != 0  return false;
    if !keepalive  return true;

    if idle_seconds_before_keepalive {
        value := idle_seconds_before_keepalive;
        if setsockopt(s, IPPROTO.TCP, TCP_KEEPIDLE, *value, size_of(s32)) != 0  return false;
    }
    if keepalive_interval_seconds {
        value := keepalive_interval_seconds;
        if setsockopt(s, IPPROTO.TCP, TCP_KEEPINTVL, *value, size_of(s32)) != 0  return false;
    }
    if num_keepalive_attempts {
        value := num_keepalive_attempts;
        if setsockopt(s, IPPROTO.TCP, TCP_KEEPCNT, *value, size_of(s32)) != 0  return false;
    }
    return true;
}

// Closes and marks the handle invalid, so that a double close is a no-op
// rather than a close of whatever descriptor was reused in the meantime.
close_and_reset :: (s: *Socket, loc := #caller_location) {
    if s.* == INVALID_SOCKET  return;
    if close(s.*) != 0 {
        log_error("%:%,%: Could not close socket descriptor %: %\n",
                  loc.fully_pathed_filename, loc.line_number, loc.character_number, s.*, errno());
    }
    s.* = INVALID_SOCKET;
}

// "1.2.3.4:80", in temporary storage.
to_string :: (addr: sockaddr_in) -> string {
    builder: String_Builder;
    builder.allocator = temp;
    append_address(*builder, addr.sin_addr.s_addr);
    print_to_builder(*builder, ":%", ntohs(addr.sin_port));
    return builder_to_string(*builder,, allocator = temp);
}

// A dotted quad. The address is in network order, so the first octet is the
// low byte.
append_address :: (builder: *String_Builder, address: u32) {
    host := ntohl(address);
    print_to_builder(*builder.*, "%.%.%.%",
                     (host >> 24) & 0xff, (host >> 16) & 0xff, (host >> 8) & 0xff, host & 0xff);
}

// `accept` over IPv6, handing back the peer's address as well.
accept_v6 :: (s: Socket) -> (accepted: Socket, address: sockaddr_in6) {
    address: sockaddr_in6;
    length: socklen_t = size_of(sockaddr_in6);
    accepted := accept(s, cast(*sockaddr) *address, *length);
    return accepted, address;
}
