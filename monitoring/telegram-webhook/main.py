#!/usr/bin/env python3
"""
Simple Alertmanager webhook receiver that posts alerts to Telegram.
"""

import os
import logging
from flask import Flask, request, jsonify
import requests

app = Flask(__name__)
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

TELEGRAM_BOT_TOKEN = os.environ.get('TELEGRAM_BOT_TOKEN')
TELEGRAM_CHAT_ID = os.environ.get('TELEGRAM_CHAT_ID')
TELEGRAM_API_URL = f'https://api.telegram.org/bot{TELEGRAM_BOT_TOKEN}/sendMessage'
HOST_HOSTNAME = os.environ.get('HOST_HOSTNAME', 'unknown')


def format_alert(alert: dict) -> str:
    """Format a single alert into a readable message."""
    status = alert.get('status', 'unknown').upper()
    labels = alert.get('labels', {})
    annotations = alert.get('annotations', {})

    alertname = labels.get('alertname', 'Unknown Alert')
    severity = labels.get('severity', 'unknown')
    instance = labels.get('instance', '')

    summary = annotations.get('summary', '')
    description = annotations.get('description', '')

    emoji = '🔴' if status == 'FIRING' else '✅'

    lines = [f"{emoji} <b>{status}: {alertname}</b>"]
    lines.append(f"Host: {HOST_HOSTNAME}")

    if severity:
        lines.append(f"Severity: {severity}")
    if instance:
        lines.append(f"Instance: {instance}")
    if summary:
        lines.append(f"\n{summary}")
    if description:
        lines.append(f"\n{description}")

    return '\n'.join(lines)


def send_telegram_message(text: str) -> bool:
    """Send a message to Telegram."""
    if not TELEGRAM_BOT_TOKEN or not TELEGRAM_CHAT_ID:
        logging.error("TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID not configured")
        return False

    try:
        response = requests.post(
            TELEGRAM_API_URL,
            json={
                'chat_id': TELEGRAM_CHAT_ID,
                'text': text,
                'parse_mode': 'HTML',
                'disable_web_page_preview': True,
            },
            timeout=10,
        )
        if response.status_code == 200:
            logging.info("Message sent to Telegram successfully")
            return True
        else:
            logging.error(f"Telegram API error: {response.status_code} - {response.text}")
            return False
    except Exception as e:
        logging.error(f"Failed to send message to Telegram: {e}")
        return False


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({'status': 'ok'})


@app.route('/', methods=['POST'])
def webhook():
    """Receive alerts from Alertmanager and forward to Telegram."""
    try:
        data = request.get_json()
        if not data:
            logging.warning("Received empty payload")
            return jsonify({'status': 'error', 'message': 'empty payload'}), 400

        alerts = data.get('alerts', [])
        if not alerts:
            logging.info("No alerts in payload")
            return jsonify({'status': 'ok', 'message': 'no alerts'})

        logging.info(f"Received {len(alerts)} alert(s)")

        # Format and send each alert
        messages = [format_alert(alert) for alert in alerts]
        full_message = '\n\n'.join(messages)

        success = send_telegram_message(full_message)

        if success:
            return jsonify({'status': 'ok'})
        else:
            return jsonify({'status': 'error', 'message': 'failed to send'}), 500

    except Exception as e:
        logging.error(f"Error processing webhook: {e}")
        return jsonify({'status': 'error', 'message': str(e)}), 500


if __name__ == '__main__':
    if not TELEGRAM_BOT_TOKEN:
        logging.error("TELEGRAM_BOT_TOKEN environment variable is required")
        exit(1)
    if not TELEGRAM_CHAT_ID:
        logging.error("TELEGRAM_CHAT_ID environment variable is required")
        exit(1)

    logging.info(f"Starting Telegram webhook receiver on port 8080")
    app.run(host='0.0.0.0', port=8080)
