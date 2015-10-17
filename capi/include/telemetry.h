#ifndef __TELEMETRY_H
#define __TELEMETRY_H

struct telemetry_t;

struct flag_t;
struct count_t;

struct telemetry_t* telemetry_init(int is_active);
void telemetry_free(struct telemetry_t*);

struct flag_t* telemetry_new_flag(const char* name);
void telemetry_add_flag(struct telemetry_t* telemetry, struct flag_t* flag);
void telemetry_record_flag(struct flag_t* flag);

struct count_t* telemetry_new_count(const char* name);
void telemetry_add_count(struct telemetry_t* telemetry, struct count_t* count);
void telemetry_record_count(struct count_t* count, unsigned int value);

char* telemetry_serialize_plain_json();
void telemetry_free_serialized_json(char* serialized);

#endif
