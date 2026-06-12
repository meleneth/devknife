FROM ruby:3.3-slim

WORKDIR /app
COPY Gemfile Gemfile.lock* ./
RUN bundle install
COPY config.ru app.rb ./

EXPOSE 9292
CMD ["bundle", "exec", "rackup", "-o", "0.0.0.0", "-p", "9292"]
