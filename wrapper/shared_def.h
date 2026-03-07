#ifndef __SHARED_DEF_H__
#define __SHARED_DEF_H__

#include <unistd.h>

#define MESSAGE_FLAG_ADD_PROCESS (1)
#define MESSAGE_FLAG_REMOVE_PROCESS (2)

struct message{
    pid_t tid;
    int flags;
};


#endif // __SHARED_DEF_H__