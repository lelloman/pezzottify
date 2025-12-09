#!/usr/bin/env python3
"""
Dynamic DNS updater for Cloudflare.

This script checks the current public IP address and updates a Cloudflare DNS
record if the IP has changed. It can also set itself up as a cron job.

Usage:
    ./update-dns.py              # Run the DNS update check
    ./update-dns.py --setup      # Install as cron job (every 5 minutes)
    ./update-dns.py --uninstall  # Remove cron job
"""

import argparse
import json
import os
import random
import subprocess
import sys
import urllib.request
import urllib.error
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent.resolve()
CONFIG_FILE = SCRIPT_DIR / "dns-config.json"
IP_CACHE_FILE = SCRIPT_DIR / ".current-ip"
CRON_COMMENT = "pezzottify-ddns-updater"


def load_config() -> dict:
    """Load configuration from the config file."""
    if not CONFIG_FILE.exists():
        print(f"Error: Config file not found: {CONFIG_FILE}")
        print(f"Please create it from dns-config.example.json")
        sys.exit(1)

    with open(CONFIG_FILE) as f:
        return json.load(f)


def get_current_ip() -> str:
    """Get the current public IP address."""
    services = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ]
    random.shuffle(services)

    for service in services:
        try:
            with urllib.request.urlopen(service, timeout=10) as response:
                ip = response.read().decode("utf-8").strip()
                if ip:
                    return ip
        except (urllib.error.URLError, TimeoutError) as e:
            print(f"Warning: Failed to get IP from {service}: {e}")
            continue

    print("Error: Could not determine current IP address from any service")
    sys.exit(1)


def get_cached_ip() -> str | None:
    """Get the previously cached IP address."""
    if IP_CACHE_FILE.exists():
        return IP_CACHE_FILE.read_text().strip()
    return None


def save_cached_ip(ip: str) -> None:
    """Save the current IP to the cache file."""
    IP_CACHE_FILE.write_text(ip)


def update_cloudflare_dns(config: dict, ip: str) -> bool:
    """Update the Cloudflare DNS record with the new IP."""
    zone_id = config["cloudflare"]["zone_id"]
    record_id = config["cloudflare"]["record_id"]
    api_token = config["cloudflare"]["api_token"]
    domain = config["cloudflare"]["domain"]

    url = (
        f"https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{record_id}"
    )

    data = json.dumps(
        {
            "type": "A",
            "name": domain,
            "content": ip,
            "ttl": config["cloudflare"].get("ttl", 300),
            "proxied": config["cloudflare"].get("proxied", False),
        }
    ).encode("utf-8")

    headers = {
        "Authorization": f"Bearer {api_token}",
        "Content-Type": "application/json",
    }

    request = urllib.request.Request(url, data=data, headers=headers, method="PUT")

    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            result = json.loads(response.read().decode("utf-8"))
            if result.get("success"):
                print(f"Successfully updated DNS record for {domain} to {ip}")
                return True
            else:
                errors = result.get("errors", [])
                print(f"Error updating DNS: {errors}")
                return False
    except urllib.error.HTTPError as e:
        error_body = e.read().decode("utf-8")
        print(f"HTTP Error {e.code}: {error_body}")
        return False
    except urllib.error.URLError as e:
        print(f"URL Error: {e}")
        return False


def setup_cron(interval_minutes: int) -> None:
    """Install this script as a cron job running at the specified interval."""
    script_path = Path(__file__).resolve()
    python_path = sys.executable

    # Create the cron entry
    cron_line = f"*/{interval_minutes} * * * * {python_path} {script_path} >> {SCRIPT_DIR}/dns-update.log 2>&1 # {CRON_COMMENT}"

    # Get current crontab
    try:
        result = subprocess.run(["crontab", "-l"], capture_output=True, text=True)
        current_crontab = result.stdout if result.returncode == 0 else ""
    except FileNotFoundError:
        print("Error: crontab command not found")
        sys.exit(1)

    # Check if already installed
    if CRON_COMMENT in current_crontab:
        print("Cron job already installed. Use --uninstall to remove it first.")
        return

    # Add new entry
    new_crontab = current_crontab.rstrip() + "\n" + cron_line + "\n"

    # Install new crontab
    process = subprocess.Popen(["crontab", "-"], stdin=subprocess.PIPE, text=True)
    process.communicate(input=new_crontab)

    if process.returncode == 0:
        print(
            f"Cron job installed successfully (runs every {interval_minutes} minutes)"
        )
        print(f"Log file: {SCRIPT_DIR}/dns-update.log")
    else:
        print("Error installing cron job")
        sys.exit(1)


def uninstall_cron() -> None:
    """Remove the cron job."""
    try:
        result = subprocess.run(["crontab", "-l"], capture_output=True, text=True)
        if result.returncode != 0:
            print("No crontab found")
            return
        current_crontab = result.stdout
    except FileNotFoundError:
        print("Error: crontab command not found")
        sys.exit(1)

    # Remove our entry
    lines = [line for line in current_crontab.splitlines() if CRON_COMMENT not in line]
    new_crontab = "\n".join(lines) + "\n" if lines else ""

    # Install modified crontab
    process = subprocess.Popen(["crontab", "-"], stdin=subprocess.PIPE, text=True)
    process.communicate(input=new_crontab)

    if process.returncode == 0:
        print("Cron job removed successfully")
    else:
        print("Error removing cron job")
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Dynamic DNS updater for Cloudflare")
    parser.add_argument("--setup", action="store_true", help="Install as cron job")
    parser.add_argument(
        "--interval",
        type=int,
        default=10,
        metavar="MINS",
        help="Cron interval in minutes (default: 10, used with --setup)",
    )
    parser.add_argument("--uninstall", action="store_true", help="Remove cron job")
    parser.add_argument(
        "--force", action="store_true", help="Force update even if IP hasn't changed"
    )
    args = parser.parse_args()

    if args.setup:
        setup_cron(args.interval)
        return

    if args.uninstall:
        uninstall_cron()
        return

    # Normal operation: check and update DNS
    config = load_config()

    current_ip = get_current_ip()
    cached_ip = get_cached_ip()

    print(f"Current IP: {current_ip}")
    print(f"Cached IP:  {cached_ip or '(none)'}")

    if current_ip == cached_ip and not args.force:
        print("IP has not changed, no update needed")
        return

    print("IP has changed, updating Cloudflare DNS...")
    if update_cloudflare_dns(config, current_ip):
        save_cached_ip(current_ip)
        print("Done!")
    else:
        print("Failed to update DNS")
        sys.exit(1)


if __name__ == "__main__":
    main()
