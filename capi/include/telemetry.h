#ifndef __TELEMETRY_H
#define __TELEMETRY_H

struct telemetry_t;

struct flag_t;
struct count_t;

struct serialized_string_t;

struct telemetry_t* telemetry_init(int is_active);
void telemetry_free(struct telemetry_t*);

struct flag_t* telemetry_new_flag(struct telemetry_t* telemetry, const char* name);
void telemetry_record_flag(struct flag_t* flag);

struct count_t* telemetry_new_count(struct telemetry_t* telemetry, const char* name);
void telemetry_record_count(struct count_t* count, unsigned int value);

struct serialized_string_t* telemetry_serialize_plain_json(struct telemetry_t* telemetry);
char* telemetry_borrow_string(struct serialized_string_t* serialized);
void telemetry_free_serialized_string(struct serialized_string_t* serialized);

#endif
