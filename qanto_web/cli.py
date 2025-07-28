import click
from .extensions import db
from .models import Question

# This decorator registers a new command 'init-db' with the flask command
@click.command('init-db')
def init_db_command():
    """Clears the existing data and creates new tables and sample questions."""
    
    # Drop all tables and recreate them to ensure a clean state
    db.drop_all()
    db.create_all()

    # Create instances of the Question model
    print("Creating sample questions...")
    q1 = Question(text="What is the capital of France?")
    q2 = Question(text="What is 2 + 2?")
    q3 = Question(text="What color is the sky?")
    q4 = Question(text="How many continents are there?")

    # Add the new questions to the database session and commit
    db.session.add_all([q1, q2, q3, q4])
    db.session.commit()
    
    print("Database has been initialized with sample questions.")
