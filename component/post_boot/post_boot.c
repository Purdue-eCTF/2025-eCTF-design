#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#define LED1 0
#define LED2 1
#define LED3 2

void LED_On(unsigned int idx);
void LED_Off(unsigned int idx);
void LED_Toggle(unsigned int idx);

void secure_send(uint8_t* buffer, uint8_t len);
int secure_receive(uint8_t* buffer);

void post_boot(void) {
    #ifdef POST_BOOT
        POST_BOOT
    #endif
}
