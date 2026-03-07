#define _GNU_SOURCE
#include "shared_def.h"
#include<dlfcn.h>
#include<mqueue.h>
#include<pthread.h>
#include<stdio.h>
#include<stdlib.h>
#include<fcntl.h>
#include <unistd.h>

typedef int (*orig_pthread_create_type)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);

static orig_pthread_create_type orig_pthread_create = NULL;
static mqd_t pid_message_queue = -1;

struct thread_data{
    void *args;
    void * (*func)(void *);
};

__attribute__((constructor))
static void constructor(){
    orig_pthread_create = dlsym(RTLD_NEXT,"pthread_create");
    if (!orig_pthread_create) {
        fprintf(stderr, "Error finding original pthread_create: %s\n", dlerror());
        exit(EXIT_FAILURE);
    }

    pid_message_queue = mq_open("/thread_stat_queue",O_WRONLY);
    if(pid_message_queue == -1){
        perror("Failed to open message queue");
        exit(EXIT_FAILURE);
    }

    pid_t tid = gettid();

    int send_return = mq_send(pid_message_queue,(char *)&tid,sizeof(tid),0);

    if(send_return == -1){
        perror("Failed to send to message queue!");
        exit(EXIT_FAILURE);
    }

}


void cleanup_handler(void *msg) {
    int send_return = mq_send(pid_message_queue,(char *)msg,sizeof(struct message),0);
    if(send_return == -1){
        perror("Failed to send to message queue!");
        exit(EXIT_FAILURE);
    }
}


static void *wrapper(void *arg) {
    struct thread_data tmp = *(struct thread_data *)arg;
    struct message msg;
    void * return_value;
    msg.flags = MESSAGE_FLAG_ADD_PROCESS;
    free(arg);
    pid_t my_pid = gettid();
    msg.tid = my_pid;
    int send_return = mq_send(pid_message_queue,(char *)&msg,sizeof(struct message),0);

    if(send_return == -1){
        perror("Failed to send to message queue!");
        exit(EXIT_FAILURE);
    }
    msg.flags = MESSAGE_FLAG_REMOVE_PROCESS;
    pthread_cleanup_push(cleanup_handler,&msg);

        return_value = tmp.func(tmp.args);

    pthread_cleanup_pop(1);

    return return_value;
}



// Wrapper for pthread_create
int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*start_routine)(void *), void *arg) {
    // Get the original pthread_create function

    struct thread_data *tmp=malloc(sizeof(struct thread_data));
    tmp->args = arg;
    tmp->func = start_routine;

    // Call the original pthread_create function
    return orig_pthread_create(thread, attr, wrapper, (void *) tmp);
}
