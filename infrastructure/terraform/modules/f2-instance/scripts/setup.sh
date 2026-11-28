#!/bin/bash

# Update existing list of packages and install some basic ones
sudo apt update
sudo apt install -y apt-transport-https ca-certificates curl software-properties-common awscli

# Handle extra EBS volume if configured
if [ -n "${extra_volume_device}" ]; then
    echo "Looking for extra EBS volume (specified as ${extra_volume_device})"

    # On newer instance types, EBS volumes appear as NVMe devices
    # Find the first unpartitioned NVMe device that's not the root volume
    EBS_DEVICE=""
    for device in /dev/nvme*n1; do
        if [ -b "$device" ]; then
            # Check if this device has partitions (root device will have partitions)
            if ! lsblk "$device" | grep -q part; then
                # Check if it's not already mounted
                if ! mount | grep -q "$device"; then
                    EBS_DEVICE="$device"
                    echo "Found EBS volume at $EBS_DEVICE"
                    break
                fi
            fi
        fi
    done

    # Fallback to the originally specified device name
    if [ -z "$EBS_DEVICE" ] && [ -b "${extra_volume_device}" ]; then
        EBS_DEVICE="${extra_volume_device}"
        echo "Using originally specified device ${extra_volume_device}"
    fi

    if [ -n "$EBS_DEVICE" ]; then
        echo "Configuring extra EBS volume at $EBS_DEVICE"

        # Check if the device is already formatted
        if ! sudo blkid "$EBS_DEVICE"; then
            echo "Formatting $EBS_DEVICE with ext4 filesystem"
            sudo mkfs.ext4 -F "$EBS_DEVICE"
        else
            echo "Device $EBS_DEVICE is already formatted"
        fi

        # Create mount point and mount the volume directly to /mnt/data
        sudo mkdir -p /mnt/data
        sudo mount "$EBS_DEVICE" /mnt/data

        # Add to fstab for persistent mounting (use UUID for reliability)
        UUID=$(sudo blkid -s UUID -o value "$EBS_DEVICE")
        if [ -n "$UUID" ] && ! grep -q "$UUID" /etc/fstab; then
            echo "UUID=$UUID /mnt/data ext4 defaults,nofail 0 2" | sudo tee -a /etc/fstab
        fi

        # Set appropriate permissions for the mount point
        sudo chown ubuntu:ubuntu /mnt/data
        sudo chmod 755 /mnt/data

        sudo mkdir -p /mnt/data/prometheus /mnt/data/grafana
        sudo chmod o+w /mnt/data/prometheus
        sudo chmod 777 /mnt/data/grafana

        echo "Extra EBS volume configured and mounted directly at /mnt/data"
    else
        echo "Warning: No suitable EBS volume found"
    fi
fi

# Set up the Docker registry
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

# Update the package list and install Docker
sudo apt update
sudo apt install -y docker-ce

# Install Vector on the host (as a systemd service) to ship logs and export host metrics
curl -fsSL https://keys.datadoghq.com/DATADOG_APT_KEY_CURRENT.public | sudo gpg --dearmor -o /usr/share/keyrings/datadog-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/datadog-archive-keyring.gpg] https://apt.vector.dev/ stable vector-0" | sudo tee /etc/apt/sources.list.d/vector.list > /dev/null

sudo apt update
sudo apt install -y vector=${vector_version}-1

# Allow `vector` to read container logs from the Docker socket
sudo usermod -aG docker vector

sudo aws s3 cp s3://${config_bucket}/vector/vector.yaml /etc/vector/vector.yaml
sudo vector validate --no-environment /etc/vector/vector.yaml
sudo systemctl enable vector
sudo systemctl restart vector

# Allow the `ubuntu` user to run `docker` commands (for SSH access)
sudo usermod -aG docker ubuntu

sudo docker network create --driver bridge internal

# Install the deploy script at a known location
cat > /home/ubuntu/deploy-version.sh <<'DEPLOY_SCRIPT_EOF'
${deploy_script}
DEPLOY_SCRIPT_EOF
chown ubuntu:ubuntu /home/ubuntu/deploy-version.sh
chmod 755 /home/ubuntu/deploy-version.sh

# Start f2
/home/ubuntu/deploy-version.sh ${tag}
