#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "telemetry.h"

int main(int argc, char* argv[]) {
  struct telemetry_t* telemetry = telemetry_init(1);

  struct flag_t* flag = telemetry_new_flag(telemetry, "FLAG");
  struct count_t* count = telemetry_new_count(telemetry, "COUNT");

  telemetry_record_flag(flag);
  telemetry_record_count(count, 2);

  struct serialized_string_t* serialized = telemetry_serialize_plain_json(telemetry);
  char* s = telemetry_borrow_string(serialized);
  assert(!strcmp(s, "{\n  \"COUNT\": 2,\n  \"FLAG\": 1\n}"));
  printf("%s\n", s);
  telemetry_free_serialized_string(serialized);

  telemetry_free(telemetry);
  return 0;
}
