from flask import Flask
from .extensions import db, migrate
from .routes import main_bp
from .models import Question
from .cli import init_db_command
import os

def create_app():
    """Create and configure the Flask application."""
    # Use os.path.dirname(__file__) to get the path of the current package
    project_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    
    # Explicitly set the instance path to be at the project root
    app = Flask(__name__, instance_path=os.path.join(project_root, 'instance'))

    # Configure the database path
    db_path = os.path.join(app.instance_path, 'qanto.db')
    app.config['SQLALCHEMY_DATABASE_URI'] = f'sqlite:///{db_path}'
    app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False

    # Ensure the instance folder exists
    try:
        os.makedirs(app.instance_path)
    except OSError:
        pass

    # Initialize extensions with the app
    db.init_app(app)
    migrate.init_app(app, db)

    # Register blueprints
    app.register_blueprint(main_bp)

    # Register the custom command with the app
    app.cli.add_command(init_db_command)

    return app
