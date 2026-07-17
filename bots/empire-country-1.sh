#!/bin/bash
set -e
cd /home/tekmage/empire-bot
python3 -u country1_bot.py --host 127.0.0.1 --port 6665 --country 1 \
  --log /home/tekmage/empire-bot/bot.log
