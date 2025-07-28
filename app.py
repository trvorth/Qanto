from qanto import create_app

app = create_app()

if __name__ == "__main__":
    # For debugging, but use a WSGI server for production
    app.run(host='localhost', port=5000, debug=True)
