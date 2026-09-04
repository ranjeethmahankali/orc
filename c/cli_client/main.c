/*
 * cli_client — CLI client for the orc server.
 *
 * Usage: cli_client <host> <port> <command> [args...]
 *
 * Commands:
 *   session start                              -> prints session_id
 *   session close <session_id>
 *   functions                                  -> prints function list
 *   constant <session_id> <type> <val>...      -> prints handle_id
 *   call <session_id> <func> <input_id>...     -> prints output_ids
 *   download <session_id> <handle_id>          -> prints type and values
 *   download_workflow <sid> <path> [output_ids...] -> writes .orc file
 *
 * Supported types for 'constant': u8 u16 u32 u64 i8 i16 i32 i64 f32 f64
 */

#include <orc_sdk/orc_sdk.h>

/* ==================== Platform socket abstraction ==================== */

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
typedef SOCKET sock_t;
typedef int    sockio_len_t;  /* send/recv length param type */
typedef int    sockio_ret_t;  /* send/recv return type */
#define SOCK_INVALID INVALID_SOCKET
#define sock_close closesocket
static int sock_init(void)
{
  WSADATA wsa;
  return WSAStartup(MAKEWORD(2, 2), &wsa);
}
static void sock_cleanup(void)
{
  WSACleanup();
}
#else
#include <arpa/inet.h>
#include <netdb.h>
#include <sys/socket.h>
#include <unistd.h>
typedef int     sock_t;
typedef size_t  sockio_len_t;  /* send/recv length param type */
typedef ssize_t sockio_ret_t;  /* send/recv return type */
#define SOCK_INVALID (-1)
#define sock_close close
static int  sock_init(void)
{
  return 0;
}
static void sock_cleanup(void) {}
#endif

/* ==================== Growable byte buffer ==================== */

typedef struct
{
  char  *data;
  size_t len;
  size_t cap;
} Buf;

static void buf_init(Buf *b)
{
  b->data = NULL;
  b->len  = 0;
  b->cap  = 0;
}

static void buf_free(Buf *b)
{
  free(b->data);
  buf_init(b);
}

static int buf_grow(Buf *b, size_t extra)
{
  size_t need = b->len + extra;
  if (need <= b->cap)
    return 0;
  size_t newcap = b->cap ? b->cap : 256;
  while (newcap < need)
    newcap *= 2;
  char *p = (char *)realloc(b->data, newcap);
  if (!p)
    return -1;
  b->data = p;
  b->cap  = newcap;
  return 0;
}

static int buf_append(Buf *b, void const *src, size_t n)
{
  if (buf_grow(b, n) != 0)
    return -1;
  memcpy(b->data + b->len, src, n);
  b->len += n;
  return 0;
}

static int buf_append_str(Buf *b, char const *s)
{
  return buf_append(b, s, strlen(s));
}

/* ==================== Minimal HTTP client ==================== */

typedef struct
{
  int    status;
  char  *body;
  size_t body_len;
} HttpResponse;

static void http_response_free(HttpResponse *r)
{
  free(r->body);
}

static sock_t tcp_connect(char const *host, uint16_t port)
{
  struct addrinfo hints, *res;
  memset(&hints, 0, sizeof(hints));
  hints.ai_family   = AF_INET;
  hints.ai_socktype = SOCK_STREAM;
  char port_str[8];
  snprintf(port_str, sizeof(port_str), "%u", (unsigned)port);
  if (getaddrinfo(host, port_str, &hints, &res) != 0)
    return SOCK_INVALID;
  sock_t s = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
  if (s == SOCK_INVALID) {
    freeaddrinfo(res);
    return SOCK_INVALID;
  }
  if (connect(s, res->ai_addr, (socklen_t)res->ai_addrlen) != 0) {
    sock_close(s);
    freeaddrinfo(res);
    return SOCK_INVALID;
  }
  freeaddrinfo(res);
  return s;
}

static int send_all(sock_t s, char const *data, size_t len)
{
  while (len > 0) {
    sockio_ret_t n = send(s, data, (sockio_len_t)len, 0);
    if (n <= 0)
      return -1;
    data += n;
    len -= (size_t)n;
  }
  return 0;
}

/* Receive the full HTTP response. Handles Content-Length. */
static int recv_response(sock_t s, HttpResponse *out)
{
  memset(out, 0, sizeof(*out));
  Buf raw;
  buf_init(&raw);
  /* Read until we have the full header (terminated by \r\n\r\n). */
  for (;;) {
    if (buf_grow(&raw, 1024) != 0) {
      buf_free(&raw);
      return -1;
    }
    sockio_ret_t n = recv(s, raw.data + raw.len, (sockio_len_t)1024, 0);
    if (n <= 0) {
      if (n == 0)
        break;
      buf_free(&raw);
      return -1;
    }
    raw.len += (size_t)n;
    /* Check if header is complete. */
    if (raw.len >= 4) {
      char *hdr_end = NULL;
      for (size_t i = 0; i <= raw.len - 4; i++) {
        if (memcmp(raw.data + i, "\r\n\r\n", 4) == 0) {
          hdr_end = raw.data + i;
          break;
        }
      }
      if (hdr_end)
        break;
    }
  }
  /* Parse status line. */
  char *hdr_end = NULL;
  for (size_t i = 0; i <= raw.len - 4; i++) {
    if (memcmp(raw.data + i, "\r\n\r\n", 4) == 0) {
      hdr_end = raw.data + i;
      break;
    }
  }
  if (!hdr_end) {
    buf_free(&raw);
    return -1;
  }
  /* Null-terminate header for string ops. */
  *hdr_end           = '\0';
  size_t header_len  = (size_t)(hdr_end - raw.data);
  size_t body_offset = header_len + 4;
  size_t body_so_far = raw.len - body_offset;
  /* Parse "HTTP/1.x NNN" */
  if (sscanf(raw.data, "HTTP/%*d.%*d %d", &out->status) != 1) {
    buf_free(&raw);
    return -1;
  }
  /* Find Content-Length. */
  size_t      content_length = 0;
  char const *cl             = strstr(raw.data, "Content-Length:");
  if (!cl)
    cl = strstr(raw.data, "content-length:");
  if (cl) {
    cl += strlen("Content-Length:");
    while (*cl == ' ')
      cl++;
    content_length = (size_t)atol(cl);
  }
  /* Read remaining body bytes. */
  while (body_so_far < content_length) {
    size_t need = content_length - body_so_far;
    if (buf_grow(&raw, need) != 0) {
      buf_free(&raw);
      return -1;
    }
    sockio_ret_t n = recv(s, raw.data + raw.len, (sockio_len_t)need, 0);
    if (n <= 0) {
      buf_free(&raw);
      return -1;
    }
    raw.len += (size_t)n;
    body_so_far += (size_t)n;
  }
  /* Copy body out. */
  out->body_len = content_length;
  out->body     = (char *)malloc(content_length + 1);
  if (!out->body) {
    buf_free(&raw);
    return -1;
  }
  memcpy(out->body, raw.data + body_offset, content_length);
  out->body[content_length] = '\0';
  buf_free(&raw);
  return 0;
}

static int http_request(char const   *host,
                        uint16_t      port,
                        char const   *method,
                        char const   *path,
                        char const   *content_type,
                        void const   *body,
                        size_t        body_len,
                        HttpResponse *out)
{
  sock_t s = tcp_connect(host, port);
  if (s == SOCK_INVALID)
    return -1;
  /* Build request line + headers. */
  Buf req;
  buf_init(&req);
  buf_append_str(&req, method);
  buf_append_str(&req, " ");
  buf_append_str(&req, path);
  buf_append_str(&req, " HTTP/1.1\r\nHost: ");
  buf_append_str(&req, host);
  buf_append_str(&req, "\r\nConnection: close\r\n");
  if (content_type) {
    buf_append_str(&req, "Content-Type: ");
    buf_append_str(&req, content_type);
    buf_append_str(&req, "\r\n");
  }
  char cl_hdr[64];
  snprintf(cl_hdr, sizeof(cl_hdr), "Content-Length: %zu\r\n", body_len);
  buf_append_str(&req, cl_hdr);
  buf_append_str(&req, "\r\n");
  if (send_all(s, req.data, req.len) != 0) {
    buf_free(&req);
    sock_close(s);
    return -1;
  }
  buf_free(&req);
  if (body_len > 0 && body) {
    if (send_all(s, (char const *)body, body_len) != 0) {
      sock_close(s);
      return -1;
    }
  }
  int rc = recv_response(s, out);
  sock_close(s);
  return rc;
}

static int http_get_json(char const   *host,
                         uint16_t      port,
                         char const   *path,
                         HttpResponse *out)
{
  return http_request(host, port, "GET", path, NULL, NULL, 0, out);
}

static int http_post_json(char const   *host,
                          uint16_t      port,
                          char const   *path,
                          char const   *json_body,
                          HttpResponse *out)
{
  return http_request(
    host, port, "POST", path, "application/json", json_body, strlen(json_body), out);
}

static int http_post_bytes(char const   *host,
                           uint16_t      port,
                           char const   *path,
                           void const   *data,
                           size_t        data_len,
                           HttpResponse *out)
{
  return http_request(
    host, port, "POST", path, "application/octet-stream", data, data_len, out);
}

/* ==================== Simple JSON value extraction ==================== */

/* Find "key": <number> in a JSON string and return the number. */
static int json_get_u64(char const *json, char const *key, uint64_t *out)
{
  char pattern[128];
  snprintf(pattern, sizeof(pattern), "\"%s\"", key);
  char const *p = strstr(json, pattern);
  if (!p)
    return -1;
  p += strlen(pattern);
  while (*p == ' ' || *p == ':')
    p++;
  char  *end;
  double val = strtod(p, &end);
  if (end == p)
    return -1;
  *out = (uint64_t)val;
  return 0;
}

/* Find "key": [n1, n2, ...] and fill array. Returns count. */
static int json_get_u64_arr(char const *json, char const *key, uint64_t *out, int max)
{
  char pattern[128];
  snprintf(pattern, sizeof(pattern), "\"%s\"", key);
  char const *p = strstr(json, pattern);
  if (!p)
    return -1;
  p += strlen(pattern);
  while (*p && *p != '[')
    p++;
  if (*p != '[')
    return -1;
  p++;
  int count = 0;
  while (*p && *p != ']' && count < max) {
    while (*p == ' ' || *p == ',')
      p++;
    if (*p == ']')
      break;
    char  *end;
    double val = strtod(p, &end);
    if (end == p)
      break;
    out[count++] = (uint64_t)val;
    p            = end;
  }
  return count;
}

/* ==================== Handle serialization via orc_sdk ==================== */

/* serial_write callback: ctx is a Buf* pointer. */
static OrcError client_serial_write(uint64_t const ctx,
                                    void const    *data,
                                    uint64_t const len)
{
  Buf *b = (Buf *)(uintptr_t)ctx;
  if (buf_append(b, data, (size_t)len) != 0)
    return ORC_ERROR_ALLOC_FAILED;
  return ORC_ERROR_NONE;
}

static void sdk_init_once(void)
{
  static bool done = false;
  if (done)
    return;
  done = true;
  OrcHost host;
  memset(&host, 0, sizeof(host));
  host.abi_version            = ORC_ABI_VERSION;
  host.callbacks.serial_write = client_serial_write;
  orc_sdk_init(&host, NULL);
}

static int serialize_handle(OrcHandle const *handle, Buf *out)
{
  uint64_t ctx = (uint64_t)(uintptr_t)out;
  OrcError err = orc_sdk_serialize_handle_header(ctx, handle);
  if (err != ORC_ERROR_NONE)
    return -1;
  if (handle->n_items > 0 && handle->items != NULL) {
    ORC_SDK_REQUIRE(handle->item_size > 0);
    err =
      orc_sdk_host_serial_write(ctx, handle->items, handle->n_items * handle->item_size);
    if (err != ORC_ERROR_NONE)
      return -1;
  }
  return 0;
}

static int deserialize_handle(void const *data, size_t data_len, OrcHandle *out)
{
  OrcStrView src;
  src.start      = (char *)data;
  src.end        = (char *)data + data_len;
  OrcMark *marks = NULL;
  OrcError err   = orc_sdk_deserialize_handle_header(0, &src, out, &marks);
  if (err != ORC_ERROR_NONE) {
    fprintf(stderr, "Deserialize header failed: 0x%x\n", err);
    return -1;
  }
  ORC_SDK_REQUIRE(out->item_size > 0);
  size_t items_bytes = (size_t)(out->n_items * out->item_size);
  void  *items       = NULL;
  if (items_bytes > 0) {
    items = malloc(items_bytes);
    if (!items) {
      orc_sdk_arr_free(marks);
      return -1;
    }
    err = orc_sdk_sv_read_bytes(&src, items, items_bytes);
    if (err != ORC_ERROR_NONE) {
      free(items);
      orc_sdk_arr_free(marks);
      return -1;
    }
  }
  out->items = items;
  out->marks = marks;
  if (!orc_sv_is_empty(src)) {
    fprintf(stderr, "Trailing bytes after deserialization\n");
    free(items);
    orc_sdk_arr_free(marks);
    return -1;
  }
  return 0;
}

static void free_deserialized_handle(OrcHandle *h)
{
  free((void *)h->items);
  OrcMark *marks = (OrcMark *)h->marks;
  orc_sdk_arr_free(marks);
  memset(h, 0, sizeof(*h));
}

/* ==================== Type name <-> type_id mapping ==================== */

typedef struct
{
  char const *name;
  OrcTypeId   type_id;
  size_t      item_size;
} TypeEntry;

static TypeEntry const TYPE_TABLE[] = {
  {"u8",  ORC_TYPE_U8,  1}, {"u16", ORC_TYPE_U16, 2}, {"u32", ORC_TYPE_U32, 4},
  {"u64", ORC_TYPE_U64, 8}, {"i8",  ORC_TYPE_I8,  1}, {"i16", ORC_TYPE_I16, 2},
  {"i32", ORC_TYPE_I32, 4}, {"i64", ORC_TYPE_I64, 8}, {"f32", ORC_TYPE_F32, 4},
  {"f64", ORC_TYPE_F64, 8},
};
#define N_TYPES (sizeof(TYPE_TABLE) / sizeof(TYPE_TABLE[0]))

static TypeEntry const *type_by_name(char const *name)
{
  for (size_t i = 0; i < N_TYPES; i++)
    if (strcmp(TYPE_TABLE[i].name, name) == 0)
      return &TYPE_TABLE[i];
  return NULL;
}

static TypeEntry const *type_by_id(OrcTypeId id)
{
  for (size_t i = 0; i < N_TYPES; i++)
    if (TYPE_TABLE[i].type_id == id)
      return &TYPE_TABLE[i];
  return NULL;
}

/* ==================== Print helpers ==================== */

static void print_handle_values(OrcHandle const *h)
{
  TypeEntry const *te = type_by_id(h->type_id);
  if (!te) {
    printf("(unknown type 0x%llx)\n", (unsigned long long)h->type_id);
    return;
  }
  for (uint64_t i = 0; i < h->n_items; i++) {
    if (i > 0) printf(" ");
    void const *p = (char const *)h->items + i * te->item_size;
    if (te->type_id == ORC_TYPE_U8)  printf("%u",   (unsigned)*(uint8_t  *)p);
    if (te->type_id == ORC_TYPE_U16) printf("%u",   (unsigned)*(uint16_t *)p);
    if (te->type_id == ORC_TYPE_U32) printf("%u",   *(uint32_t *)p);
    if (te->type_id == ORC_TYPE_U64) printf("%llu", (unsigned long long)*(uint64_t *)p);
    if (te->type_id == ORC_TYPE_I8)  printf("%d",   (int)*(int8_t  *)p);
    if (te->type_id == ORC_TYPE_I16) printf("%d",   (int)*(int16_t *)p);
    if (te->type_id == ORC_TYPE_I32) printf("%d",   *(int32_t *)p);
    if (te->type_id == ORC_TYPE_I64) printf("%lld", (long long)*(int64_t *)p);
    if (te->type_id == ORC_TYPE_F32) printf("%g",   (double)*(float *)p);
    if (te->type_id == ORC_TYPE_F64) printf("%g",   *(double *)p);
  }
  printf("\n");
}

/* ==================== Usage ==================== */

static void usage(void)
{
  fprintf(stderr,
    "Usage: cli_client <host> <port> <command> [args...]\n"
    "\n"
    "Commands:\n"
    "  session start                              Print session_id\n"
    "  session close <session_id>                 Close session\n"
    "  functions                                  Print function list\n"
    "  constant <session_id> <type> <val>...      Print handle_id\n"
    "  call <session_id> <func> <input_id>...     Print output handle_ids\n"
    "  download <session_id> <handle_id>          Print type and values\n"
    "  download_workflow <sid> <path> [ids...]     Write workflow to file\n"
    "\n"
    "Types: u8 u16 u32 u64 i8 i16 i32 i64 f32 f64\n");
  exit(1);
}

static void die(char const *msg)
{
  fprintf(stderr, "ERROR: %s\n", msg);
  exit(1);
}

/* ==================== Command implementations ==================== */

static void cmd_session_start(char const *host, uint16_t port)
{
  HttpResponse resp;
  if (http_post_json(host, port, "/session/start", "{}", &resp) != 0)
    die("POST /session/start failed");
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("session start failed");
  }
  uint64_t session_id;
  if (json_get_u64(resp.body, "session_id", &session_id) != 0)
    die("Failed to parse session_id");
  printf("%llu\n", (unsigned long long)session_id);
  http_response_free(&resp);
}

static void cmd_session_close(char const *host, uint16_t port, char const *sid_str)
{
  char body[128];
  snprintf(body, sizeof(body), "{\"session_id\": %s}", sid_str);
  HttpResponse resp;
  if (http_post_json(host, port, "/session/close", body, &resp) != 0)
    die("POST /session/close failed");
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("session close failed");
  }
  http_response_free(&resp);
}

static void cmd_functions(char const *host, uint16_t port)
{
  HttpResponse resp;
  if (http_get_json(host, port, "/functions", &resp) != 0)
    die("GET /functions failed");
  printf("%s\n", resp.body);
  http_response_free(&resp);
}

static void cmd_constant(char const *host,
                          uint16_t    port,
                          char const *sid_str,
                          char const *type_name,
                          int         n_values,
                          char      **value_strs)
{
  TypeEntry const *te = type_by_name(type_name);
  if (!te) {
    fprintf(stderr, "Unknown type: %s\n", type_name);
    usage();
  }
  /* Parse values into a raw buffer. */
  void *items = malloc(te->item_size * (size_t)n_values);
  if (!items) die("alloc failed");
  for (int i = 0; i < n_values; i++) {
    void *dst = (char *)items + (size_t)i * te->item_size;
    double v  = strtod(value_strs[i], NULL);
    switch (te->type_id) {
      case ORC_TYPE_U8:  *(uint8_t  *)dst = (uint8_t)v;  break;
      case ORC_TYPE_U16: *(uint16_t *)dst = (uint16_t)v;  break;
      case ORC_TYPE_U32: *(uint32_t *)dst = (uint32_t)v;  break;
      case ORC_TYPE_U64: *(uint64_t *)dst = (uint64_t)v;  break;
      case ORC_TYPE_I8:  *(int8_t   *)dst = (int8_t)v;   break;
      case ORC_TYPE_I16: *(int16_t  *)dst = (int16_t)v;  break;
      case ORC_TYPE_I32: *(int32_t  *)dst = (int32_t)v;  break;
      case ORC_TYPE_I64: *(int64_t  *)dst = (int64_t)v;  break;
      case ORC_TYPE_F32: *(float    *)dst = (float)v;    break;
      case ORC_TYPE_F64: *(double   *)dst = v;           break;
      default: break;
    }
  }
  OrcHandle handle;
  memset(&handle, 0, sizeof(handle));
  handle.type_id   = te->type_id;
  handle.n_items   = (uint64_t)n_values;
  handle.item_size = (uint64_t)te->item_size;
  handle.items     = items;
  Buf ser;
  buf_init(&ser);
  if (serialize_handle(&handle, &ser) != 0) {
    free(items);
    die("Failed to serialize handle");
  }
  free(items);
  char path[256];
  snprintf(path, sizeof(path), "/constant?session_id=%s", sid_str);
  HttpResponse resp;
  if (http_post_bytes(host, port, path, ser.data, ser.len, &resp) != 0) {
    buf_free(&ser);
    die("POST /constant failed");
  }
  buf_free(&ser);
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("constant failed");
  }
  uint64_t handle_id;
  if (json_get_u64(resp.body, "handle_id", &handle_id) != 0)
    die("Failed to parse handle_id");
  printf("%llu\n", (unsigned long long)handle_id);
  http_response_free(&resp);
}

static void cmd_call(char const *host,
                      uint16_t    port,
                      char const *sid_str,
                      char const *func_name,
                      int         n_inputs,
                      char      **input_strs)
{
  Buf body;
  buf_init(&body);
  buf_append_str(&body, "{\"session_id\": ");
  buf_append_str(&body, sid_str);
  buf_append_str(&body, ", \"function\": \"");
  buf_append_str(&body, func_name);
  buf_append_str(&body, "\", \"inputs\": [");
  for (int i = 0; i < n_inputs; i++) {
    if (i > 0) buf_append_str(&body, ", ");
    buf_append_str(&body, input_strs[i]);
  }
  buf_append_str(&body, "]}");
  /* Null-terminate for http_post_json. */
  buf_append(&body, "\0", 1);
  HttpResponse resp;
  if (http_post_json(host, port, "/call", body.data, &resp) != 0) {
    buf_free(&body);
    die("POST /call failed");
  }
  buf_free(&body);
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("call failed");
  }
  uint64_t output_ids[64];
  int n_outputs = json_get_u64_arr(resp.body, "output_ids", output_ids, 64);
  for (int i = 0; i < n_outputs; i++) {
    if (i > 0) printf(" ");
    printf("%llu", (unsigned long long)output_ids[i]);
  }
  printf("\n");
  http_response_free(&resp);
}

static void cmd_download(char const *host,
                          uint16_t    port,
                          char const *sid_str,
                          char const *hid_str)
{
  char path[256];
  snprintf(path, sizeof(path), "/download?session_id=%s&handle_id=%s", sid_str, hid_str);
  HttpResponse resp;
  if (http_post_bytes(host, port, path, NULL, 0, &resp) != 0)
    die("POST /download failed");
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("download failed");
  }
  OrcHandle result;
  memset(&result, 0, sizeof(result));
  if (deserialize_handle(resp.body, resp.body_len, &result) != 0) {
    http_response_free(&resp);
    die("Failed to deserialize result");
  }
  http_response_free(&resp);
  TypeEntry const *te = type_by_id(result.type_id);
  printf("%s ", te ? te->name : "unknown");
  print_handle_values(&result);
  free_deserialized_handle(&result);
}

static void cmd_download_workflow(char const *host,
                                   uint16_t    port,
                                   char const *sid_str,
                                   char const *out_path,
                                   int         n_outputs,
                                   char      **output_strs)
{
  char path[256];
  snprintf(path, sizeof(path),
           "/download_workflow?session_id=%s", sid_str);
  Buf body;
  buf_init(&body);
  buf_append_str(&body, "{\"outputs\": [");
  for (int i = 0; i < n_outputs; i++) {
    if (i > 0) buf_append_str(&body, ", ");
    buf_append_str(&body, output_strs[i]);
  }
  buf_append_str(&body, "]}");
  buf_append(&body, "\0", 1);
  HttpResponse resp;
  if (http_post_json(host, port, path, body.data, &resp) != 0) {
    buf_free(&body);
    die("POST /download_workflow failed");
  }
  buf_free(&body);
  if (resp.status != 200) {
    fprintf(stderr, "%s\n", resp.body);
    http_response_free(&resp);
    die("download_workflow failed");
  }
  FILE *f = fopen(out_path, "wb");
  if (!f) {
    http_response_free(&resp);
    die("Failed to open output file");
  }
  fwrite(resp.body, 1, resp.body_len, f);
  fclose(f);
  http_response_free(&resp);
}

/* ==================== Main ==================== */

int main(int argc, char **argv)
{
  if (argc < 4) usage();
  char const *host = argv[1];
  uint16_t    port = (uint16_t)atoi(argv[2]);
  char const *cmd  = argv[3];

  sdk_init_once();
  if (sock_init() != 0)
    die("Failed to initialize sockets");

  if (strcmp(cmd, "session") == 0) {
    if (argc < 5) usage();
    if (strcmp(argv[4], "start") == 0) {
      cmd_session_start(host, port);
    } else if (strcmp(argv[4], "close") == 0) {
      if (argc < 6) usage();
      cmd_session_close(host, port, argv[5]);
    } else {
      usage();
    }
  } else if (strcmp(cmd, "functions") == 0) {
    cmd_functions(host, port);
  } else if (strcmp(cmd, "constant") == 0) {
    if (argc < 7) usage();  /* host port constant sid type val... */
    cmd_constant(host, port, argv[4], argv[5], argc - 6, &argv[6]);
  } else if (strcmp(cmd, "call") == 0) {
    if (argc < 6) usage();  /* host port call sid func [inputs...] */
    cmd_call(host, port, argv[4], argv[5], argc - 6, &argv[6]);
  } else if (strcmp(cmd, "download") == 0) {
    if (argc < 6) usage();  /* host port download sid hid */
    cmd_download(host, port, argv[4], argv[5]);
  } else if (strcmp(cmd, "download_workflow") == 0) {
    if (argc < 6) usage();  /* host port download_workflow sid outpath [output_ids...] */
    cmd_download_workflow(host, port, argv[4], argv[5], argc - 6, &argv[6]);
  } else {
    usage();
  }

  sock_cleanup();
  return 0;
}
