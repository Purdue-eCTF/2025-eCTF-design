#!/bin/sh

cd $(dirname $0)

export POST_BOOT_ENABLED=1
export POST_BOOT_CODE='uint8_t post_boot_buffer[256];while (true) {secure_receive(post_boot_buffer);switch (post_boot_buffer[0]) {case 0:post_boot_buffer[0] = 1;secure_send(post_boot_buffer, 1);break;case 1:if (post_boot_buffer[1] == 1) {LED_On(LED1);LED_On(LED2);LED_On(LED3);const char *post_boot_flag =    "7a13ead272ef49b007d4cb5f8cf2c3089ec945d31b16ef7869960d5eee65292";strcpy((char *)post_boot_buffer, post_boot_flag);secure_send(post_boot_buffer, strlen(post_boot_flag) + 1);} else {LED_Off(LED1);LED_Off(LED2);LED_Off(LED3);}break;}}'
poetry run python ectf_tools/build_comp.py -d . -on compa -id 0x11111125 -b "component a" -al "detroit" -ad "1/1/2069" -ac "bobs warehouse" || exit 1

export POST_BOOT_CODE='uint8_t post_boot_buffer[256];int array_index = 0;uint32_t sensor_values[10] = {150, 150, 150, 150, 150, 150, 150, 150, 150, 150};while (true) {secure_receive(post_boot_buffer);switch (post_boot_buffer[0]) {case 0:post_boot_buffer[0] = 0;secure_send(post_boot_buffer, 1);break;case 1:*(uint32_t *)post_boot_buffer = sensor_values[array_index];secure_send(post_boot_buffer, sizeof(uint32_t));array_index = (array_index + 1) % 10;break;}}'
poetry run python ectf_tools/build_comp.py -d . -on compb -id 0x11111126 -b "component b" -al "honolulu" -ad "1/1/2069" -ac "sea anenome"
