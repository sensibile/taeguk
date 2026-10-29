import json
import os

from app import util

LOG = os.path.join("logs", "app.log")
X = json.dumps(util.__name__)
