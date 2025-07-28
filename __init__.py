from flask import Flask
from .extensions import db, migrate
from .routes import main_bp
from .models import Question
from .cli import init_db_command # Import the new command
import os

def create_app():
    """Create and configure the Flask application."""
    app = Flask(__name__, instance_relative_config=True)

    db_path = os.path.join(app.instance_path, 'qanto.db')
    app.config['SQLALCHEMY_DATABASE_URI'] = f'sqlite:///{db_path}'
    app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False

    try:
        os.makedirs(app.instance_path)
    except OSError:
        pass

    db.init_app(app)
    migrate.init_app(app, db)

    app.register_blueprint(main_bp)

    # Register the custom command with the app
    app.cli.add_command(init_db_command)

    return app
