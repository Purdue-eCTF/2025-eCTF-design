#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

void MXC_Delay(uint32_t us);

typedef uint8_t i2c_addr_t;

int secure_send(uint8_t address, uint8_t* buffer, uint8_t len);
int secure_receive(i2c_addr_t address, uint8_t* buffer);
int get_provisioned_ids(uint32_t* buffer);

void post_boot(void){
    #ifdef POST_BOOT
        POST_BOOT
    #endif
}