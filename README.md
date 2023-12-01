# 🌡️🛜 thermoscan

`thermoscan` is a small <200 lines of code program that passively monitors the temperature of off-the-shelf themometers and sends the data to a Loki instance.

`thermoscan` works by listening for BLE advertisements from the thermometers and parsing the data from the advertisements. This way the thermometers do not need to be paired with the device running `thermoscan` and no alterations to the thermometers are needed.

`thermoscan` is designed to run on a Raspberry Pi Zero W, but should work on any Linux device with a BLE adapter, and currently supports parsing data from the GOVEE H5075 thermometers.

# Whats this for?

`thermoscan` is a small part of a larger project to monitor the temperature of my house. I'd like to optimize the heating and cooling of my house to save money and energy, and to do that I need to know the temperature of each room. 

### Current setup

The total setup cost for this project can be as little as $25 (1 RPI Zero W + 1 thermometer) and can scale up to as many thermometers as you need. Adding a new one is as simple as buying it and turning it on.

- 1 Raspberry Pi Zero W running `thermoscan` (~$15)
- 7 GOVEE H5075 thermometers (~$10 each)
- 1 free Grafana Cloud account

# RPI setup

This code was written to work with the Raspberry Pi OS (port of Debian Bullseye) image installed with the default `Raspberry Pi Imager` tool. The only changes made were adding our local wifi network in the settings and enabling SSH (couple clicks in the UI).

# Setup, Build and Sync

```bash
# install cross compilation tools
cargo install cross --git https://github.com/cross-rs/cross

# build for raspberry pi
cross build --target arm-unknown-linux-gnueabihf --features rpi 

# copy to raspberry pi over local network
rsync -r target/arm-unknown-linux-gnueabihf/debug/thermoscan pi@raspberrypi.local:/home/pi/thermoscan-dev

# copy config file to raspberry pi over local network
rsync -r .env pi@raspberrypi.local:/home/pi/.env
```

Now that it's on the pi, we can run it with `./thermoscan-dev` and see the output. If everything looks good, we can setup a systemd service to run it on startup. This lets us run it headless and not have to worry about it, we can unplug it and move it around and it will just work.

# Setup startup service

```bash
# set permissions and move service file
sudo chmod 644 thermoscan.service

# move service file to systemd folder
mv thermoscan.service /lib/systemd/system/

# reload systemd daemon and enable service
sudo systemctl daemon-reload
sudo systemctl enable thermoscan.service

# reboot the whole thing
sudo reboot
```
